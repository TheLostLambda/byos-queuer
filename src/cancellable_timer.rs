// Standard Library Imports
use std::{
    sync::{Condvar, Mutex, MutexGuard, RwLock, RwLockReadGuard, RwLockWriteGuard, TryLockError},
    time::{Duration, Instant},
};

// Public API ==========================================================================================================

pub struct CancellableTimer {
    start: Mutex<Instant>,
    duration: Duration,
    cancelled: RwLock<bool>,
    condvar: Condvar,
    condvar_mutex: Mutex<()>,
}

impl CancellableTimer {
    #[must_use]
    pub const fn new(start: Instant, duration: Duration) -> Self {
        let start = Mutex::new(start);
        let cancelled = RwLock::new(false);
        let condvar = Condvar::new();
        let condvar_mutex = Mutex::new(());

        Self {
            start,
            duration,
            cancelled,
            condvar,
            condvar_mutex,
        }
    }

    #[must_use]
    pub const fn duration(&self) -> Duration {
        self.duration
    }

    #[must_use]
    pub fn cancelled(&self) -> bool {
        *self.cancelled.read().unwrap()
    }

    pub fn cancel(&self) {
        // NOTE: Along with `self.cancelled_writer()`, this prevents `self.cancelled` from changing between
        // `.wait_timeout_while()` and the dropping of `TimerGuard` in `self.wait()`
        let _condvar_lock = self.condvar_mutex.lock().unwrap();

        *self.cancelled_writer() = true;
        self.condvar.notify_all();
    }

    pub fn resume(&self) {
        // NOTE: Along with `self.cancelled_writer()`, this prevents `self.cancelled` from changing between
        // `.wait_timeout_while()` and the dropping of `TimerGuard` in `self.wait()`
        let _condvar_lock = self.condvar_mutex.lock().unwrap();

        *self.cancelled_writer() = false;
    }

    pub fn wait(&self) -> Option<TimerGuard<'_>> {
        let start_lock = self.start.lock().unwrap();
        let elapsed = start_lock.elapsed();
        let duration_to_go = self.duration.saturating_sub(elapsed);

        let (condvar_lock, result) = self
            .condvar
            .wait_timeout_while(self.condvar_mutex.lock().unwrap(), duration_to_go, |()| {
                !*self.cancelled.read().unwrap()
            })
            .unwrap();

        // NOTE: This ordering ensures that at least one of `cancelled` or `condvar_mutex` is locked continuously
        // from the return of `.wait_timeout_while()` until the `TimerGuard` is dropped. This prevents `self.cancel()`
        // and `self.resume()` from being called at any time within this "locking-window". The only method of
        // `CancellableTimer` that can be called in this window is `self.cancelled()`
        let cancelled_read = self.cancelled.read().unwrap();
        drop(condvar_lock);

        result
            .timed_out()
            .then_some(TimerGuard::new(start_lock, cancelled_read))
    }
}

pub struct TimerGuard<'a> {
    start_lock: MutexGuard<'a, Instant>,
    _cancelled_read: RwLockReadGuard<'a, bool>,
}

impl TimerGuard<'_> {
    pub fn reset(mut self) -> Instant {
        *self.start_lock = Instant::now();
        *self.start_lock
    }
}

// Private Helper Code =================================================================================================

impl CancellableTimer {
    // NOTE: This is a vile hack to get around the deadlock described in the documentation for `RwLock` — in short, it's
    // default behaviour for `RwLock.read()` to block if another `RwLock.write()` call is waiting somewhere, even if
    // there is already a `RwLockReadGuard` floating around somewhere! This is to avoid writer starvation, but is
    // causing a deadlock in my code. I'll hack my way around that by just spinning with `RwLock.try_write()` which
    // doesn't block like `RwLock.write()` does
    fn cancelled_writer(&self) -> RwLockWriteGuard<'_, bool> {
        loop {
            match self.cancelled.try_write() {
                Ok(write_guard) => return write_guard,
                Err(TryLockError::WouldBlock) => {}
                _ => panic!(),
            }
        }
    }
}

impl<'a> TimerGuard<'a> {
    const fn new(
        start_lock: MutexGuard<'a, Instant>,
        cancelled_read: RwLockReadGuard<'a, bool>,
    ) -> Self {
        Self {
            start_lock,
            // NOTE: It's important that we hold this lock open, but we never actually need to access its contents via
            // `TimerGuard`, so it's marked with an underscore (`_`) to stop Rust from complaining that it's unused
            _cancelled_read: cancelled_read,
        }
    }
}

// Unit Tests ==========================================================================================================

#[cfg(test)]
mod tests {
    use std::thread;

    use crate::worker_pool::tests::sleep_ms;

    use super::*;

    #[test]
    // NOTE: We're showing off how the `guard` can be used to reset the timer after running — I want to hold onto it in
    // a variable so that this example is a bit clearer!
    #[expect(clippy::significant_drop_tightening)]
    fn wait_and_reset() {
        let start = Instant::now();
        let timer = CancellableTimer::new(start, Duration::from_millis(5));

        let guard = timer.wait().unwrap();
        let elapsed = start.elapsed();
        assert!(Duration::from_millis(3) < elapsed && elapsed < Duration::from_millis(7));

        // NOTE: Dropping this is important, otherwise the next call to `wait()` deadlocks!
        drop(guard);

        let start = Instant::now();

        // Waiting for the timer again returns instantly!
        let guard = timer.wait().unwrap();
        assert!(start.elapsed() < Duration::from_micros(50));

        // But the timer can be reset using its `TimerGuard`
        let start = guard.reset();

        timer.wait().unwrap();
        let elapsed = start.elapsed();
        assert!(Duration::from_millis(3) < elapsed && elapsed < Duration::from_millis(7));
    }

    #[test]
    fn cancel_timer() {
        let start = Instant::now();
        let timer = CancellableTimer::new(start, Duration::from_millis(10));

        thread::scope(|s| {
            let thread_handles: Vec<_> = (0..2)
                .map(|_| {
                    s.spawn(|| {
                        let start_of_wait = Instant::now();
                        if let Some(guard) = timer.wait() {
                            guard.reset();
                        }
                        start_of_wait.elapsed()
                    })
                })
                .collect();

            sleep_ms(5);
            timer.cancel();

            for elapsed in thread_handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
            {
                assert!(Duration::from_millis(3) < elapsed && elapsed < Duration::from_millis(7));
            }
        });

        // Timer refuses to wait whilst it's cancelled
        let start = Instant::now();
        assert!(timer.wait().is_none());
        assert!(start.elapsed() < Duration::from_micros(50));

        // But the timer can be resumed / uncancelled
        timer.resume();
        let start = Instant::now();

        timer.wait().unwrap();
        let elapsed = start.elapsed();
        assert!(Duration::from_millis(3) < elapsed && elapsed < Duration::from_millis(7));

        // And finally, show that timers can still finish on their own without cancelling
        timer.wait().unwrap().reset();

        thread::scope(|s| {
            let thread_handles: Vec<_> = (0..2)
                .map(|_| {
                    s.spawn(|| {
                        let start_of_wait = Instant::now();
                        if let Some(guard) = timer.wait() {
                            guard.reset();
                        }
                        start_of_wait.elapsed()
                    })
                })
                .collect();

            sleep_ms(15);
            timer.cancel();

            let thread_durations: Vec<_> = thread_handles
                .into_iter()
                .map(|handle| handle.join().unwrap())
                .collect();

            let elapsed = thread_durations[0];
            assert!(Duration::from_millis(8) < elapsed && elapsed < Duration::from_millis(12));

            let elapsed = thread_durations[1];
            assert!(Duration::from_millis(13) < elapsed && elapsed < Duration::from_millis(17));
        });
    }
}
