// Standard Library Imports
use std::{
    sync::{Condvar, Mutex, MutexGuard},
    time::{Duration, Instant},
};

// Public API ==========================================================================================================

pub struct CancellableTimer {
    start: Mutex<Instant>,
    duration: Duration,
    cancelled: Mutex<bool>,
    condvar: Condvar,
}

impl CancellableTimer {
    #[must_use]
    pub const fn new(start: Instant, duration: Duration) -> Self {
        let start = Mutex::new(start);
        let cancelled = Mutex::new(false);
        let condvar = Condvar::new();

        Self {
            start,
            duration,
            cancelled,
            condvar,
        }
    }

    #[must_use]
    pub const fn duration(&self) -> Duration {
        self.duration
    }

    #[must_use]
    pub fn cancelled(&self) -> bool {
        *self.cancelled.lock().unwrap()
    }

    pub fn cancel(&self) {
        *self.cancelled.lock().unwrap() = true;
        self.condvar.notify_all();
    }

    pub fn resume(&self) {
        *self.cancelled.lock().unwrap() = false;
    }

    pub fn wait(&self) -> Option<TimerGuard<'_>> {
        let start_lock = self.start.lock().unwrap();
        let elapsed = start_lock.elapsed();
        let duration_to_go = self.duration.saturating_sub(elapsed);

        let (lock, result) = self
            .condvar
            .wait_timeout_while(
                self.cancelled.lock().unwrap(),
                duration_to_go,
                |&mut cancelled| !cancelled,
            )
            .unwrap();
        drop(lock);

        result.timed_out().then_some(TimerGuard(start_lock))
    }
}

pub struct TimerGuard<'a>(MutexGuard<'a, Instant>);

impl TimerGuard<'_> {
    pub fn reset(mut self) -> Instant {
        *self.0 = Instant::now();
        *self.0
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
