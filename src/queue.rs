use std::{
    sync::Mutex,
    time::{Duration, Instant},
};

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

pub struct Queue {
    jobs: Mutex<Vec<Job>>,
    worker_pool: WorkerPool,
}

impl Queue {
    pub fn new(workers: usize) -> Result<Self> {
        let jobs = Mutex::new(Vec::new());
        let worker_pool = WorkerPool::new(workers)?;

        Ok(Self { jobs, worker_pool })
    }

    // TODO: If the `Queue` is already running, then any new workflows added to the queue are started immediately!
    // Otherwise, just add them to `jobs` and wait for `Queue.run()` to be called! This variable should be `true`
    // as long as anything in the `jobs` list isn't finished (i.e. `Queued` or `Running`)
    pub fn queue(&mut self) -> Result<()> {
        todo!()
    }

    // TODO: Have a `queue` and `queue_grouped` option, where the `queue` method automatically splits into single-sample
    // `Workflow`s!
    pub fn queue_grouped(&mut self) -> Result<()> {
        todo!()
    }

    pub fn run(&mut self) {
        todo!()
    }
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
