// Standard Library Imports
use std::{
    sync::{
        Arc,
        atomic::{AtomicUsize, Ordering},
    },
    thread,
};

// External Crate Imports
use color_eyre::{Result, eyre::eyre};

// Local Crate Imports
use crate::on_update::OnUpdate;

// Public API ==========================================================================================================

#[derive(Debug)]
pub struct WorkerPool {
    workers: Arc<AtomicUsize>,
    max_workers: usize,
    on_update: OnUpdate,
}

impl WorkerPool {
    pub fn new(max_workers: usize, on_update: OnUpdate) -> Result<Self> {
        if max_workers == 0 {
            return Err(eyre!("a `WorkerPool` must allow at least one worker"));
        }

        let workers = Arc::new(AtomicUsize::new(0));

        Ok(Self {
            workers,
            max_workers,
            on_update,
        })
    }

    #[must_use]
    pub fn running(&self) -> bool {
        self.workers.load(Ordering::Relaxed) != 0
    }

    #[must_use]
    pub fn available_workers(&self) -> usize {
        self.max_workers - self.workers.load(Ordering::Relaxed)
    }

    pub fn spawn(
        &self,
        new_workers: usize,
        process: impl FnOnce() + Clone + Send + 'static,
    ) -> Result<usize> {
        let previous_workers = self.workers
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |mut workers| {
                workers += new_workers;
                (workers <= self.max_workers).then_some(workers)
            })
            .map_err(|workers|
                eyre!("tried to launch {new_workers} new workers, but {workers} workers were already running \
                       and the maximum number of workers is {}", self.max_workers))?;

        // NOTE: If we've gone from no workers to `self.running()`, then the `WorkerPool` has just been started and we
        // should trigger an `on_update()`
        if previous_workers == 0 && self.running() {
            self.on_update.call();
        }

        for _ in 0..new_workers {
            let workers = Arc::clone(&self.workers);
            let on_update = self.on_update.clone();
            let process = process.clone();
            thread::spawn(move || {
                process();

                let previous_workers = workers.fetch_sub(1, Ordering::Relaxed);

                // NOTE: If this thread was the only worker and is now closing, then the `WorkerPool` has just stopped
                // and we should trigger an `on_update()`
                if previous_workers == 1 {
                    on_update.call();
                }
            });
        }

        Ok(self.workers.load(Ordering::Relaxed))
    }
}

// Unit Tests ==========================================================================================================

#[cfg(test)]
pub(crate) mod tests {
    use std::{
        sync::{Condvar, Mutex, WaitTimeoutResult},
        time::Duration,
    };

    use super::*;

    pub fn sleep_ms(millis: u64) {
        thread::sleep(Duration::from_millis(millis));
    }

    #[derive(Debug, Default)]
    pub struct ThreadParker {
        unpark_count: Mutex<usize>,
        condvar: Condvar,
    }

    impl ThreadParker {
        pub fn new() -> Self {
            let unpark_count = Mutex::new(0);
            let condvar = Condvar::new();

            Self {
                unpark_count,
                condvar,
            }
        }

        pub fn unpark(&self) {
            *self.unpark_count.lock().unwrap() += 1;
            self.condvar.notify_all();
        }

        pub fn park_timeout(&self, timeout: Duration) -> WaitTimeoutResult {
            let (mut unpark_count, result) = self
                .condvar
                .wait_timeout_while(
                    self.unpark_count.lock().unwrap(),
                    timeout,
                    |&mut unpark_count| unpark_count == 0,
                )
                .unwrap();

            if !result.timed_out() {
                // SAFETY: This should never underflow since the `.wait_timeout_while()` returning *without* timing out
                // means that `unpark_count == 0` became false (meaning that `unpark_count > 0` since it's a `usize`).
                // Because `.wait_timeout_while()` returns an already-locked `unpark_count`, we can also be certain that
                // no thread will have changed that value in the meantime
                *unpark_count -= 1;
            }
            drop(unpark_count);

            result
        }

        pub fn no_missed_parks(&self) -> bool {
            *self.unpark_count.lock().unwrap() == 0
        }
    }

    #[macro_export]
    macro_rules! assert_unpark_within_ms {
        ($thread_parker:expr, $timeout:expr $(,)?) => {
            let timeout = Duration::from_millis($timeout);
            assert!(
                !$thread_parker.park_timeout(timeout).timed_out(),
                "the thread did not unpark within {timeout:?}"
            );
        };
    }

    #[test]
    fn new() {
        let worker_pool = WorkerPool::new(0, OnUpdate::new());
        assert!(worker_pool.is_err());
        assert_eq!(
            worker_pool.unwrap_err().to_string(),
            "a `WorkerPool` must allow at least one worker"
        );

        let worker_pool = WorkerPool::new(1, OnUpdate::new()).unwrap();
        assert!(!worker_pool.running());
        assert_eq!(worker_pool.available_workers(), 1);

        let worker_pool = WorkerPool::new(6, OnUpdate::new()).unwrap();
        assert!(!worker_pool.running());
        assert_eq!(worker_pool.available_workers(), 6);
    }

    #[test]
    fn spawn() {
        let worker_pool = WorkerPool::new(6, OnUpdate::new()).unwrap();
        assert!(!worker_pool.running());
        assert_eq!(worker_pool.available_workers(), 6);

        let active_workers = worker_pool.spawn(1, || sleep_ms(5)).unwrap();
        assert!(worker_pool.running());
        assert_eq!(worker_pool.available_workers(), 5);
        assert_eq!(active_workers, 1);

        sleep_ms(3);

        let active_workers = worker_pool.spawn(5, || sleep_ms(5)).unwrap();
        assert!(worker_pool.running());
        assert_eq!(worker_pool.available_workers(), 0);
        assert_eq!(active_workers, 6);

        sleep_ms(3);

        assert!(worker_pool.running());
        assert_eq!(worker_pool.available_workers(), 1);

        let active_workers = worker_pool.spawn(2, || sleep_ms(5));
        assert!(active_workers.is_err());
        assert_eq!(
            active_workers.unwrap_err().to_string(),
            "tried to launch 2 new workers, but 5 workers were already running and the maximum number of workers is 6"
        );

        sleep_ms(3);

        assert!(!worker_pool.running());
        assert_eq!(worker_pool.available_workers(), 6);
    }

    #[test]
    fn on_update_callback() {
        // Construct a `WorkerPool` with a callback
        let on_update = OnUpdate::new();
        let worker_pool = Arc::new(WorkerPool::new(6, on_update.clone()).unwrap());

        let thread_parker = Arc::new(ThreadParker::new());
        let updates = Arc::new(Mutex::new(Vec::new()));
        let on_update_callback = Arc::new({
            let updates = Arc::clone(&updates);
            let worker_pool = Arc::clone(&worker_pool);
            let thread_parker = Arc::clone(&thread_parker);
            move || {
                updates.lock().unwrap().push(worker_pool.running());
                thread_parker.unpark();
            }
        });

        on_update.set(on_update_callback);

        // Start the `WorkerPool` and make sure the callback is being invoked!
        let active_workers = worker_pool.spawn(5, || sleep_ms(5)).unwrap();

        assert_unpark_within_ms!(thread_parker, 1);
        assert!(worker_pool.running());
        assert_eq!(worker_pool.available_workers(), 1);
        assert_eq!(active_workers, 5);

        let active_workers = worker_pool.spawn(1, || sleep_ms(5)).unwrap();

        assert!(worker_pool.running());
        assert_eq!(worker_pool.available_workers(), 0);
        assert_eq!(active_workers, 6);

        assert_unpark_within_ms!(thread_parker, 6);
        assert!(!worker_pool.running());
        assert_eq!(worker_pool.available_workers(), 6);

        // Make sure the `WorkerPool` isn't started by `.spawn()`ing zero workers
        let active_workers = worker_pool.spawn(0, || sleep_ms(5)).unwrap();

        assert!(!worker_pool.running());
        assert_eq!(worker_pool.available_workers(), 6);
        assert_eq!(active_workers, 0);

        // Start the `WorkerPool` again, but don't let it finish before checking `updates`
        let active_workers = worker_pool.spawn(2, || sleep_ms(5)).unwrap();

        assert_unpark_within_ms!(thread_parker, 1);
        assert!(worker_pool.running());
        assert_eq!(worker_pool.available_workers(), 4);
        assert_eq!(active_workers, 2);

        assert_eq!(updates.lock().unwrap()[..], [true, false, true]);

        // Let the `WorkerPool` finish and then check `updates` again
        assert_unpark_within_ms!(thread_parker, 6);

        assert_eq!(updates.lock().unwrap()[..], [true, false, true, false]);

        assert!(thread_parker.no_missed_parks());
    }
}
