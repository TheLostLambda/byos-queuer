use std::time::{Duration, Instant};

use color_eyre::eyre::Result;

use crate::{worker_pool::WorkerPool, workflow::Workflow};

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

struct Queue {
    jobs: Vec<Job>,
    thread_pool: WorkerPool,
    // TODO: If the `Queue` is already running, then any new workflows added to the queue are started immediately!
    // Otherwise, just add them to `jobs` and wait for `Queue.run()` to be called! This variable should be `true`
    // as long as anything in the `jobs` list isn't finished (i.e. `Queued` or `Running`)
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
