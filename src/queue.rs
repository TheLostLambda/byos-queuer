use std::{
    sync::{
        Arc, RwLock,
        atomic::{AtomicBool, AtomicUsize, Ordering},
    },
    thread::{self, JoinHandle, ThreadId},
    time::{Duration, Instant},
};

use color_eyre::eyre::Result;

use crate::workflow::Workflow;

enum Status {
    Queued,
    Running(Instant),
    Completed(Duration),
    Failed(Duration),
}

struct Job {
    workflow: Workflow,
    status: Status,
}

// FIXME: Should this be split out into it's own `thread_pool.rs` module?
// FIXME: Should be called DynamicThreadPool? Because its size changes?
struct ThreadPool {
    threads: Arc<AtomicUsize>,
    max_threads: usize,
}

impl ThreadPool {
    fn new(max_threads: usize) -> Self {
        Self {
            threads: Arc::new(AtomicUsize::new(0)),
            max_threads,
        }
    }

    fn available_threads(&self) -> usize {
        self.max_threads - self.threads.load(Ordering::Relaxed)
    }

    // TODO: write `spawn_all`?
    fn spawn<T: Send + 'static>(
        &self,
        process: impl FnOnce() -> T + Send + 'static,
    ) -> Option<JoinHandle<T>> {
        self.threads
            .fetch_update(Ordering::Relaxed, Ordering::Relaxed, |mut threads| {
                threads += 1;
                (threads <= self.max_threads).then_some(threads)
            })
            .ok()?;

        let threads = Arc::clone(&self.threads);
        let handle = thread::spawn(move || {
            let result = process();
            threads.fetch_sub(1, Ordering::Relaxed);
            result
        });

        Some(handle)
    }
}

struct Queue {
    jobs: Vec<Job>,
    thread_pool: ThreadPool,
    // TODO: If the `Queue` is already running, then any new workflows added to the queue are started immediately!
    // Otherwise, just add them to `jobs` and wait for `Queue.run()` to be called! This variable should be `true`
    // as long as anything in the `jobs` list isn't finished (i.e. `Queued` or `Running`)
    running: AtomicBool,
}

impl Queue {
    fn new(threads: Option<usize>) -> Result<Self> {
        todo!()
    }

    // TODO: `Self::queue(Workflow)` that just immediately spawns things into the pool, but also adds them to the list
    // of jobs so that we have a history of what's been queued
    //
    // Use `spawn_fifo`?

    // TODO: Have a `queue` and `queue_grouped` option, where the `queue` method automatically splits into single-sample
    // `Workflow`s!
}

#[cfg(test)]
mod tests {
    use super::*;

    #[ignore]
    #[test]
    fn new() {
        todo!()
    }
}
