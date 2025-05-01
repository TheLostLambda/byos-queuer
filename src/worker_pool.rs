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

// Public API ==========================================================================================================

#[derive(Debug)]
pub struct WorkerPool {
    workers: Arc<AtomicUsize>,
    max_workers: usize,
}

impl WorkerPool {
    pub fn new(max_workers: usize) -> Result<Self> {
        if max_workers == 0 {
            return Err(eyre!("a `WorkerPool` must allow at least one worker"));
        }

        let workers = Arc::new(AtomicUsize::new(0));

        Ok(Self {
            workers,
            max_workers,
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
        self.workers
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |mut workers| {
                workers += new_workers;
                (workers <= self.max_workers).then_some(workers)
            })
            .map_err(|workers|
                eyre!("tried to launch {new_workers} new workers, but {workers} workers were already running \
                       and the maximum number of workers is {}", self.max_workers))?;

        for _ in 0..new_workers {
            let workers = Arc::clone(&self.workers);
            let process = process.clone();
            thread::spawn(move || {
                process();
                workers.fetch_sub(1, Ordering::Relaxed);
            });
        }

        Ok(self.workers.load(Ordering::Relaxed))
    }
}

// Unit Tests ==========================================================================================================

#[cfg(test)]
mod tests {
    use std::time::Duration;

    use super::*;

    fn sleep_ms(millis: u64) {
        thread::sleep(Duration::from_millis(millis));
    }

    #[test]
    fn new() {
        let worker_pool = WorkerPool::new(0);
        assert!(worker_pool.is_err());
        assert_eq!(
            worker_pool.unwrap_err().to_string(),
            "a `WorkerPool` must allow at least one worker"
        );

        let worker_pool = WorkerPool::new(1).unwrap();
        assert!(!worker_pool.running());
        assert_eq!(worker_pool.available_workers(), 1);

        let worker_pool = WorkerPool::new(6).unwrap();
        assert!(!worker_pool.running());
        assert_eq!(worker_pool.available_workers(), 6);
    }

    #[test]
    fn spawn() {
        let worker_pool = WorkerPool::new(6).unwrap();
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
}
