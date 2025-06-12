// Standard Library Imports
use std::{
    cmp,
    path::Path,
    sync::{Arc, RwLock},
    time::{Duration, Instant},
};

// External Crate Imports
use color_eyre::eyre::{OptionExt, Result, eyre};

// Local Crate Imports
use crate::{
    cancellable_timer::CancellableTimer,
    job::{Job, Status as JobStatus},
    on_update::{OnUpdate, OnUpdateCallback},
    worker_pool::WorkerPool,
    workflow::Workflow,
};

// Public API ==========================================================================================================

pub struct Queue {
    jobs: Jobs,
    worker_pool: WorkerPool,
    stagger_timer: StaggerTimer,
    on_update: OnUpdate,
}

#[derive(Copy, Clone, Eq, PartialEq, Debug)]
pub enum Status {
    /// The `Queue` is stopped and contains no `Job`s
    Empty,
    /// The `Queue` is stopped and contains only `Queued` `Job`s
    Ready,
    /// The `Queue` is running but contains no `Running` `Job`s
    Starting,
    /// The `Queue` is running and contains at least one `Running` `Job`
    Running,
    /// The `Queue` is still running but has been cancelled or contains only finished (`Completed` / `Failed`) `Job`s
    Stopping,
    /// The `Queue` is stopped and contains both finished (`Completed` / `Failed`) and `Queued` `Job`s
    Paused,
    /// The `Queue` is stopped and contains only finished (`Completed` / `Failed`) `Job`s
    Finished,
}

impl Queue {
    pub fn new(workers: usize, stagger_duration: Duration) -> Result<Self> {
        let on_update = OnUpdate::new();

        let jobs = Arc::new(RwLock::new(Vec::new()));
        let worker_pool = WorkerPool::new(workers, on_update.clone())?;
        let stagger_timer = Self::stagger_timer(stagger_duration)?;

        Ok(Self {
            jobs,
            worker_pool,
            stagger_timer,
            on_update,
        })
    }

    // Setters ---------------------------------------------------------------------------------------------------------

    pub fn set_workers(&mut self, workers: usize) -> Result<()> {
        self.error_if_running("set_workers")?;

        self.worker_pool = WorkerPool::new(workers, self.on_update.clone())?;

        Ok(())
    }

    pub fn set_stagger_duration(&mut self, stagger_duration: Duration) -> Result<()> {
        self.error_if_running("set_stagger_duration")?;

        self.stagger_timer = Self::stagger_timer(stagger_duration)?;

        Ok(())
    }

    pub fn set_on_update(&self, on_update: OnUpdateCallback) {
        self.on_update.set(on_update);
    }

    pub fn clear_on_update(&self) {
        self.on_update.clear();
    }

    // Getters ---------------------------------------------------------------------------------------------------------

    #[must_use]
    pub fn status(&self) -> Status {
        // NOTE: This prevents any other thread from obtaining write-access to `self.jobs` whilst this method is
        // running. This prevents other threads from changing `self.jobs` between the first and second half of an `&&`
        // expression, which could otherwise lead to us reaching the `unreachable!()` branch
        let _jobs_guard = self.jobs.read().unwrap();
        if self.workers_running() && (self.cancelled() || self.only_finished_jobs()) {
            Status::Stopping
        } else if self.workers_running() && self.no_running_jobs() {
            Status::Starting
        } else if self.workers_running() {
            Status::Running
        } else if self.no_jobs() {
            Status::Empty
        } else if self.only_finished_jobs() {
            Status::Finished
        } else if self.partially_finished_jobs() {
            Status::Paused
        } else if self.only_queued_jobs() {
            Status::Ready
        } else {
            unreachable!();
        }
    }

    #[must_use]
    pub fn jobs(&self) -> Vec<(String, JobStatus)> {
        self.jobs
            .read()
            .unwrap()
            .iter()
            .map(|job| (job.name().to_owned(), job.status()))
            .collect()
    }

    // Job Control -----------------------------------------------------------------------------------------------------

    pub fn queue_jobs(
        &self,
        base_workflow: impl AsRef<Path> + Copy,
        sample_files: impl IntoIterator<Item = impl AsRef<Path>> + Copy,
        protein_file: impl AsRef<Path> + Copy,
        modifications_file: Option<impl AsRef<Path> + Copy>,
        output_directory: impl AsRef<Path> + Copy,
    ) -> Result<()> {
        for sample_file in sample_files {
            self.queue_job(
                base_workflow,
                &[sample_file],
                protein_file,
                modifications_file,
                output_directory,
            )?;
        }

        self.on_update.call();

        Ok(())
    }

    pub fn queue_grouped_job(
        &self,
        base_workflow: impl AsRef<Path> + Copy,
        sample_files: impl IntoIterator<Item = impl AsRef<Path>> + Copy,
        protein_file: impl AsRef<Path> + Copy,
        modifications_file: Option<impl AsRef<Path> + Copy>,
        output_directory: impl AsRef<Path> + Copy,
    ) -> Result<()> {
        self.queue_job(
            base_workflow,
            sample_files,
            protein_file,
            modifications_file,
            output_directory,
        )?;

        self.on_update.call();

        Ok(())
    }

    pub fn remove_job(&self, index: usize) -> Result<()> {
        // NOTE: We need to hold this lock open to ensure that the length of the list cannot be changed between the
        // bounds-check and the actual call to `.remove()`, otherwise a panic is possible
        let mut jobs_guard = self.jobs.write().unwrap();

        let jobs_len = jobs_guard.len();
        let removed_job = if index < jobs_len {
            jobs_guard.remove(index)
        } else {
            return Err(eyre!(
                "tried to remove the `Job` at index {index}, but there are only {jobs_len} `Job`s in the `Queue`"
            ));
        };
        drop(jobs_guard);

        // NOTE: This kills any `removed_job`s that were still running
        removed_job.abandon()?;

        self.on_update.call();

        Ok(())
    }

    pub fn reset_job(&self, index: usize) -> Result<()> {
        let job_to_reset = self.jobs.read().unwrap().get(index).cloned();

        if let Some(job_to_reset) = job_to_reset {
            job_to_reset.reset()?;

            // NOTE: Resetting a `Job` effectively re-queues it — just like ordinary queuing, this re-queuing might
            // require spawning a new worker
            self.spawn_worker_if_running();

            Ok(())
        } else {
            let jobs_len = self.jobs.read().unwrap().len();

            Err(eyre!(
                "tried to reset the `Job` at index {index}, but there are only {jobs_len} `Job`s in the `Queue`"
            ))
        }
    }

    // Queue Control ---------------------------------------------------------------------------------------------------

    pub fn run(&self) -> Result<()> {
        self.stagger_timer.resume();

        let queued_jobs = self
            .jobs
            .read()
            .unwrap()
            .iter()
            .filter(|job| matches!(job.status(), JobStatus::Queued))
            .count();
        let available_workers = self.worker_pool.available_workers();
        let new_workers = cmp::min(queued_jobs, available_workers);

        self.spawn_workers(new_workers)?;

        Ok(())
    }

    pub fn cancel(&self) -> Result<()> {
        self.stagger_timer.cancel();

        for job in self.jobs.read().unwrap().iter() {
            if let JobStatus::Running(..) = job.status() {
                job.quiet_reset()?;
            }
        }

        self.on_update.call();

        Ok(())
    }

    pub fn clear(&self) -> Result<()> {
        self.stagger_timer.cancel();

        for job in self.jobs.write().unwrap().drain(..) {
            // NOTE: This will ensure that any still-running, `.drain()`ed `Job`s are quietly killed
            job.abandon()?;
        }

        self.on_update.call();

        Ok(())
    }

    pub fn reset(&self) -> Result<()> {
        for job in self.jobs.read().unwrap().iter() {
            job.quiet_reset()?;
            // NOTE: Resetting a `Job` effectively re-queues it — just like ordinary queuing, this re-queuing might
            // require spawning a new worker
            self.spawn_worker_if_running();
        }

        self.on_update.call();

        Ok(())
    }
}

// Private Helper Code =================================================================================================

type Jobs = Arc<RwLock<Vec<Arc<Job>>>>;
type StaggerTimer = Arc<CancellableTimer>;

impl Queue {
    fn stagger_timer(stagger_duration: Duration) -> Result<StaggerTimer> {
        // NOTE: By setting this `Instant` to be `stagger_duration` in the past, we're ensuring no launch delay for the
        // first job — the `stagger_duration` will have already "passed" by the time the `CancellableTimer` is
        // constructed
        let start = Instant::now()
            .checked_sub(stagger_duration)
            .ok_or_eyre("`stagger_duration` was too large")?;

        Ok(Arc::new(CancellableTimer::new(start, stagger_duration)))
    }

    fn error_if_running(&self, method_name: &str) -> Result<()> {
        if self.workers_running() {
            return Err(eyre!("the `Queue` must be stopped to `{method_name}()`"));
        }

        Ok(())
    }

    fn workers_running(&self) -> bool {
        self.worker_pool.running()
    }

    fn cancelled(&self) -> bool {
        self.stagger_timer.cancelled()
    }

    fn only_finished_jobs(&self) -> bool {
        self.jobs.read().unwrap().iter().all(|job| {
            matches!(
                job.status(),
                JobStatus::Completed(..) | JobStatus::Failed(..)
            )
        })
    }

    fn no_running_jobs(&self) -> bool {
        !self
            .jobs
            .read()
            .unwrap()
            .iter()
            .any(|job| matches!(job.status(), JobStatus::Running(..)))
    }

    fn no_jobs(&self) -> bool {
        self.jobs.read().unwrap().is_empty()
    }

    fn partially_finished_jobs(&self) -> bool {
        // DESIGN: More imparative approach here so that I can look for both types of `Job` in a single pass and break
        // out of the loop as soon as I've found one of each. This is like `Iterator::any()` but looking for multiple
        // predicates at once
        let mut contains_queued = false;
        let mut contains_finished = false;

        for job in self.jobs.read().unwrap().iter() {
            let status = job.status();

            if !contains_queued && matches!(status, JobStatus::Queued) {
                contains_queued = true;
            } else if !contains_finished
                && matches!(status, JobStatus::Completed(..) | JobStatus::Failed(..))
            {
                contains_finished = true;
            }

            if contains_queued && contains_finished {
                return true;
            }
        }

        false
    }

    fn only_queued_jobs(&self) -> bool {
        self.jobs
            .read()
            .unwrap()
            .iter()
            .all(|job| matches!(job.status(), JobStatus::Queued))
    }

    pub fn queue_job(
        &self,
        base_workflow: impl AsRef<Path> + Copy,
        sample_files: impl IntoIterator<Item = impl AsRef<Path>> + Copy,
        protein_file: impl AsRef<Path> + Copy,
        modifications_file: Option<impl AsRef<Path> + Copy>,
        output_directory: impl AsRef<Path> + Copy,
    ) -> Result<()> {
        let workflow = Workflow::new(
            base_workflow,
            sample_files,
            protein_file,
            modifications_file,
            output_directory,
        )?;

        self.jobs
            .write()
            .unwrap()
            .push(Arc::new(Job::new(workflow, self.on_update.clone())));

        // NOTE: If the `Queue` is already running, but isn't at its maximum worker capacity yet, then a new worker may
        // need spawning — this method does nothing if the `Queue` is stopped or already at its maximum worker capacity.
        self.spawn_worker_if_running();

        Ok(())
    }

    fn spawn_workers(&self, new_workers: usize) -> Result<()> {
        let jobs = Arc::clone(&self.jobs);
        let stagger_timer = Arc::clone(&self.stagger_timer);

        self.worker_pool.spawn(new_workers, move || {
            Self::worker(&jobs, &stagger_timer);
        })?;

        Ok(())
    }

    fn spawn_worker_if_running(&self) {
        if self.workers_running() && !self.cancelled() {
            // NOTE: `WorkerPool.spawn()` will check if there is room in the pool for another worker, if there isn't,
            // then it will return an `Err`. We don't actually mind this outcome, so just discard the `Result` with a
            // `let _ = ...` binding.
            let _ = self.spawn_workers(1);
        }
    }

    // -----------------------------------------------------------------------------------------------------------------

    fn worker(jobs: &Jobs, stagger_timer: &StaggerTimer) {
        let next_job_staggered = || {
            let Some(timer_guard) = stagger_timer.wait() else {
                // NOTE: If we *didn't* time out, then this `Queue` must have been cancelled — return `None` to break
                // out of the worker loop and shutdown the thread
                return None;
            };

            Self::next_job(jobs).map(|job| (job, timer_guard))
        };

        while let Some((job, timer_guard)) = next_job_staggered() {
            // SAFETY: The call to `next_job_staggered()` should only ever return `JobStatus::Queued` `Job`s, so
            // `Job.run()` shouldn't ever fail!
            job.start().unwrap();

            // NOTE: Dropping this lock here finally allows other threads to grab another `Job` — it's important this
            // lock is dropped *after* `job.start()` is called, since `job.start()` is what sets the `Job` status to
            // `Running`. If this lock is dropped any earlier, then it's possible for several threads to start on the
            // same `Job`, since the `Job` status will still be `Queued`!
            timer_guard.reset();

            job.wait();
        }
    }

    // -----------------------------------------------------------------------------------------------------------------

    fn next_job(jobs: &Jobs) -> Option<Arc<Job>> {
        jobs.read()
            .unwrap()
            .iter()
            .find(|job| matches!(job.status(), JobStatus::Queued))
            .cloned()
    }
}

// Unit Tests ==========================================================================================================

#[cfg(test)]
mod tests {
    use std::{sync::Mutex, thread};

    use tempfile::tempdir;

    use crate::{
        assert_unpark_within_ms,
        job::StatusDiscriminant::{self as JobStatusDiscriminant, *},
        worker_pool::tests::{ThreadParker, sleep_ms},
        workflow::tests::{
            BASE_WORKFLOW, MODIFICATIONS_FILE, PROTEIN_FASTA_FILE, SAMPLE_FILES, WORKFLOW_NAME,
            with_test_path,
        },
    };

    use super::*;

    const WT_WORKFLOW_NAME: &str = "PG Monomers (WT; proteins.fasta; modifications.txt)";
    const LDT_WORKFLOW_NAME: &str = "PG Monomers (6ldt; proteins.fasta; modifications.txt)";

    const FAST_PATH: &str = "tests/scripts/queue-fast";
    const SLOW_PATH: &str = "tests/scripts/queue-slow";

    fn job_statuses(queue: &Queue) -> Vec<JobStatusDiscriminant> {
        queue
            .jobs()
            .into_iter()
            .map(|(_, status)| status.into())
            .collect()
    }

    fn sleep_until_elapsed_ms(instant: Instant, millis: u64) {
        let target = Duration::from_millis(millis);
        let elapsed = instant.elapsed();
        let duration_to_go = target.saturating_sub(elapsed);

        thread::sleep(duration_to_go);
    }

    #[test]
    fn queue() {
        let temporary_directory = tempdir().unwrap();
        let queue = Queue::new(1, Duration::default()).unwrap();

        queue
            .queue_jobs(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                &temporary_directory,
            )
            .unwrap();

        let jobs = queue.jobs();

        let (name, status) = &jobs[0];
        assert_eq!(name, WT_WORKFLOW_NAME);
        assert!(matches!(status, JobStatus::Queued));

        let (name, status) = &jobs[1];
        assert_eq!(name, LDT_WORKFLOW_NAME);
        assert!(matches!(status, JobStatus::Queued));
    }

    #[test]
    fn queue_grouped() {
        let temporary_directory = tempdir().unwrap();
        let queue = Queue::new(1, Duration::default()).unwrap();

        queue
            .queue_grouped_job(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                &temporary_directory,
            )
            .unwrap();

        let jobs = queue.jobs();

        let (name, status) = &jobs[0];
        assert_eq!(name, WORKFLOW_NAME);
        assert!(matches!(status, JobStatus::Queued));
    }

    #[test]
    fn run_checking_staggering() {
        let temporary_directory = tempdir().unwrap();

        // First, queue some `Job`s
        let queue = Queue::new(3, Duration::from_millis(10)).unwrap();

        assert_eq!(queue.status(), Status::Empty);

        queue
            .queue_jobs(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                &temporary_directory,
            )
            .unwrap();

        queue
            .queue_grouped_job(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                &temporary_directory,
            )
            .unwrap();

        assert_eq!(queue.status(), Status::Ready);

        let expected_job_names = [WT_WORKFLOW_NAME, LDT_WORKFLOW_NAME, WORKFLOW_NAME];
        let jobs = queue.jobs();
        let job_names: Vec<_> = jobs.iter().map(|(name, _)| name).collect();
        assert_eq!(job_names, expected_job_names);
        assert_eq!(job_statuses(&queue), [Queued, Queued, Queued]);

        // Then, make sure they run staggered and in parallel!
        let test_code = || {
            queue.run().unwrap();

            assert_eq!(queue.status(), Status::Starting);

            sleep_ms(5);

            assert_eq!(queue.status(), Status::Running);
            assert_eq!(job_statuses(&queue), [Running, Queued, Queued]);

            sleep_ms(15);

            assert_eq!(job_statuses(&queue), [Running, Running, Queued]);

            sleep_ms(15);

            assert_eq!(job_statuses(&queue), [Running, Running, Running]);

            sleep_ms(20);

            assert_eq!(job_statuses(&queue), [Failed, Running, Running]);

            sleep_ms(20);

            assert_eq!(job_statuses(&queue), [Failed, Running, Completed]);

            sleep_ms(20);

            assert_eq!(job_statuses(&queue), [Failed, Completed, Completed]);

            assert_eq!(queue.status(), Status::Finished);
        };

        unsafe { with_test_path(FAST_PATH, test_code) }
    }

    #[test]
    fn run_checking_addition_to_running_queue() {
        let temporary_directory = tempdir().unwrap();

        // First, queue an initial `Job`
        let queue = Queue::new(2, Duration::from_millis(100)).unwrap();

        queue
            .queue_grouped_job(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                &temporary_directory,
            )
            .unwrap();

        assert_eq!(queue.status(), Status::Ready);
        assert_eq!(queue.worker_pool.available_workers(), 2);
        assert_eq!(job_statuses(&queue), [Queued]);

        // Then, run the queue and add some more `Job`s whilst it's running!
        let test_code = || {
            queue.run().unwrap();

            assert_eq!(queue.status(), Status::Starting);

            sleep_ms(5);

            assert_eq!(queue.status(), Status::Running);
            assert_eq!(queue.worker_pool.available_workers(), 1);
            assert_eq!(job_statuses(&queue), [Running]);

            let instant = Instant::now();
            queue
                .queue_jobs(
                    BASE_WORKFLOW,
                    SAMPLE_FILES,
                    PROTEIN_FASTA_FILE,
                    Some(MODIFICATIONS_FILE),
                    &temporary_directory,
                )
                .unwrap();

            sleep_until_elapsed_ms(instant, 230);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Running, Running, Queued]);

            sleep_ms(40);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Completed, Running, Running]);

            sleep_ms(170);

            assert_eq!(queue.worker_pool.available_workers(), 1);
            assert_eq!(job_statuses(&queue), [Completed, Completed, Running]);

            sleep_ms(40);

            assert_eq!(queue.status(), Status::Finished);
            assert_eq!(queue.worker_pool.available_workers(), 2);
            assert_eq!(job_statuses(&queue), [Completed, Completed, Completed]);
        };

        unsafe { with_test_path(SLOW_PATH, test_code) }
    }

    #[test]
    // NOTE: It's a test, so I think this is alright for now
    #[expect(clippy::too_many_lines)]
    fn reconfigure_when_stopped() {
        let temporary_directory = tempdir().unwrap();

        // First, queue an initial couple of `Job`s
        let mut queue = Queue::new(1, Duration::from_millis(60)).unwrap();

        for _ in 0..2 {
            queue
                .queue_grouped_job(
                    BASE_WORKFLOW,
                    SAMPLE_FILES,
                    PROTEIN_FASTA_FILE,
                    Some(MODIFICATIONS_FILE),
                    &temporary_directory,
                )
                .unwrap();
        }

        assert_eq!(queue.status(), Status::Ready);
        assert_eq!(queue.worker_pool.available_workers(), 1);
        assert_eq!(job_statuses(&queue), [Queued, Queued]);

        // Then, run the queue and make sure it cannot be reconfigured whilst running
        let test_code = || {
            queue.run().unwrap();

            assert_eq!(queue.status(), Status::Starting);

            sleep_ms(5);

            assert_eq!(queue.status(), Status::Running);
            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Running, Queued]);

            sleep_ms(50);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Completed, Queued]);

            assert_eq!(
                queue.set_workers(3).unwrap_err().to_string(),
                "the `Queue` must be stopped to `set_workers()`"
            );
            assert_eq!(
                queue
                    .set_stagger_duration(Duration::default())
                    .unwrap_err()
                    .to_string(),
                "the `Queue` must be stopped to `set_stagger_duration()`"
            );

            sleep_ms(20);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Completed, Running]);

            sleep_ms(55);

            assert_eq!(queue.worker_pool.available_workers(), 1);
            assert_eq!(job_statuses(&queue), [Completed, Completed]);

            assert_eq!(queue.status(), Status::Finished);
        };

        unsafe { with_test_path(FAST_PATH, test_code) }

        // Reconfigure the queue and add some new `Job`s

        queue.set_workers(3).unwrap();
        queue.set_stagger_duration(Duration::default()).unwrap();

        queue
            .queue_jobs(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                &temporary_directory,
            )
            .unwrap();

        assert_eq!(queue.status(), Status::Paused);

        // Run the queue again, making sure that our changes were applied and that `Job` history is retained
        let test_code = || {
            queue.run().unwrap();

            assert_eq!(queue.status(), Status::Starting);

            sleep_ms(10);

            assert_eq!(queue.status(), Status::Running);
            assert_eq!(queue.worker_pool.available_workers(), 1);
            assert_eq!(
                job_statuses(&queue),
                [Completed, Completed, Running, Running]
            );

            sleep_ms(50);

            assert_eq!(queue.worker_pool.available_workers(), 2);
            assert_eq!(
                job_statuses(&queue),
                [Completed, Completed, Failed, Running]
            );

            assert_eq!(
                queue.set_workers(3).unwrap_err().to_string(),
                "the `Queue` must be stopped to `set_workers()`"
            );
            assert_eq!(
                queue
                    .set_stagger_duration(Duration::default())
                    .unwrap_err()
                    .to_string(),
                "the `Queue` must be stopped to `set_stagger_duration()`"
            );

            sleep_ms(30);

            assert_eq!(queue.worker_pool.available_workers(), 3);
            assert_eq!(
                job_statuses(&queue),
                [Completed, Completed, Failed, Completed]
            );

            assert_eq!(queue.status(), Status::Finished);
        };

        unsafe { with_test_path(FAST_PATH, test_code) }

        assert_eq!(
            job_statuses(&queue),
            [Completed, Completed, Failed, Completed]
        );

        queue.reset().unwrap();

        assert_eq!(job_statuses(&queue), [Queued, Queued, Queued, Queued]);

        queue.clear().unwrap();

        assert!(job_statuses(&queue).is_empty());
    }

    #[test]
    fn cancel() {
        let temporary_directory = tempdir().unwrap();

        // First, queue some initial `Job`s
        let queue = Queue::new(2, Duration::from_millis(20)).unwrap();

        queue
            .queue_grouped_job(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                &temporary_directory,
            )
            .unwrap();

        queue
            .queue_jobs(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                &temporary_directory,
            )
            .unwrap();

        assert_eq!(queue.status(), Status::Ready);
        assert_eq!(queue.worker_pool.available_workers(), 2);
        assert_eq!(job_statuses(&queue), [Queued, Queued, Queued]);

        // Then, run the queue and stop it whilst it's running!
        let test_code = || {
            queue.run().unwrap();

            assert_eq!(queue.status(), Status::Starting);

            sleep_ms(5);

            assert_eq!(queue.status(), Status::Running);
            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Running, Queued, Queued]);

            sleep_ms(25);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Running, Running, Queued]);

            sleep_ms(25);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Completed, Running, Running]);

            queue.cancel().unwrap();

            sleep_ms(5);

            assert_eq!(queue.status(), Status::Paused);
            assert!(queue.cancelled());
            assert_eq!(queue.worker_pool.available_workers(), 2);
            assert_eq!(job_statuses(&queue), [Completed, Queued, Queued]);
        };

        unsafe { with_test_path(FAST_PATH, test_code) }
    }

    #[test]
    fn remove_job() {
        let temporary_directory = tempdir().unwrap();

        // First, queue some `Job`s
        let queue = Queue::new(2, Duration::from_millis(10)).unwrap();

        queue
            .queue_grouped_job(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                &temporary_directory,
            )
            .unwrap();

        queue
            .queue_jobs(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                &temporary_directory,
            )
            .unwrap();

        assert_eq!(queue.status(), Status::Ready);
        assert_eq!(queue.worker_pool.available_workers(), 2);
        assert_eq!(job_statuses(&queue), [Queued, Queued, Queued]);

        // Then, make sure `Job`s can be removed whilst it's running
        let test_code = || {
            queue.run().unwrap();

            assert_eq!(queue.status(), Status::Starting);

            sleep_ms(5);

            assert_eq!(queue.status(), Status::Running);
            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Running, Queued, Queued]);

            queue.remove_job(1).unwrap();

            assert_eq!(queue.status(), Status::Running);
            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Running, Queued]);

            sleep_ms(15);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Running, Running]);

            queue.remove_job(0).unwrap();

            assert_eq!(queue.status(), Status::Running);
            assert_eq!(job_statuses(&queue), [Running]);

            sleep_ms(5);

            assert_eq!(queue.worker_pool.available_workers(), 1);

            sleep_ms(50);

            assert_eq!(queue.worker_pool.available_workers(), 1);
            assert_eq!(job_statuses(&queue), [Running]);

            sleep_ms(15);

            assert_eq!(queue.worker_pool.available_workers(), 2);
            assert_eq!(job_statuses(&queue), [Completed]);

            assert_eq!(queue.status(), Status::Finished);

            queue.remove_job(0).unwrap();
            assert_eq!(job_statuses(&queue), []);

            assert_eq!(
                queue.remove_job(0).unwrap_err().to_string(),
                "tried to remove the `Job` at index 0, but there are only 0 `Job`s in the `Queue`"
            );
        };

        unsafe { with_test_path(FAST_PATH, test_code) }
    }

    #[test]
    fn reset_job() {
        let temporary_directory = tempdir().unwrap();

        // First, queue some initial `Job`s
        let queue = Queue::new(2, Duration::from_millis(20)).unwrap();

        queue
            .queue_grouped_job(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                &temporary_directory,
            )
            .unwrap();

        queue
            .queue_jobs(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                &temporary_directory,
            )
            .unwrap();

        assert_eq!(queue.status(), Status::Ready);
        assert_eq!(queue.worker_pool.available_workers(), 2);
        assert_eq!(job_statuses(&queue), [Queued, Queued, Queued]);

        // Then, run the queue and reset some jobs whilst it's running and after it's finished
        // NOTE: It's a test, so I think this is alright for now
        #[expect(clippy::cognitive_complexity)]
        let test_code = || {
            queue.run().unwrap();

            assert_eq!(queue.status(), Status::Starting);

            sleep_ms(5);

            assert_eq!(queue.status(), Status::Running);
            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Running, Queued, Queued]);

            sleep_ms(25);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Running, Running, Queued]);

            queue.reset_job(0).unwrap();

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Stopping, Running, Queued]);

            sleep_ms(5);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Queued, Running, Queued]);

            sleep_ms(20);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Running, Running, Queued]);

            sleep_ms(25);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Running, Failed, Running]);

            sleep_ms(20);

            assert_eq!(queue.worker_pool.available_workers(), 1);
            assert_eq!(job_statuses(&queue), [Completed, Failed, Running]);

            queue.reset_job(0).unwrap();

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Queued, Failed, Running]);

            sleep_ms(5);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Running, Failed, Running]);

            sleep_ms(50);

            assert_eq!(queue.status(), Status::Finished);
            assert_eq!(queue.worker_pool.available_workers(), 2);
            assert_eq!(job_statuses(&queue), [Completed, Failed, Completed]);

            queue.reset_job(1).unwrap();

            assert_eq!(queue.status(), Status::Paused);
            assert_eq!(queue.worker_pool.available_workers(), 2);
            assert_eq!(job_statuses(&queue), [Completed, Queued, Completed]);

            assert_eq!(
                queue.reset_job(4).unwrap_err().to_string(),
                "tried to reset the `Job` at index 4, but there are only 3 `Job`s in the `Queue`"
            );
        };

        unsafe { with_test_path(FAST_PATH, test_code) }
    }

    #[test]
    // NOTE: It's a test, so I think this is alright for now
    #[expect(clippy::too_many_lines)]
    #[expect(clippy::cognitive_complexity)]
    // NOTE: Implementing the suggestions here lead to the test not compiling
    #[expect(clippy::significant_drop_tightening)]
    fn on_update_callback() {
        let temporary_directory = tempdir().unwrap();

        // First, set up a `Queue` with a debugging / logging callback
        let queue = Arc::new(Queue::new(3, Duration::from_millis(10)).unwrap());

        let thread_parker = Arc::new(ThreadParker::new());
        let updates = Arc::new(Mutex::new(Vec::new()));
        let on_update = Arc::new({
            let updates = Arc::clone(&updates);
            let queue = Arc::clone(&queue);
            let thread_parker = Arc::clone(&thread_parker);
            move || {
                updates
                    .lock()
                    .unwrap()
                    .push((job_statuses(&queue), queue.status()));
                thread_parker.unpark();
            }
        });

        // Start by queueing a couple of `Job`s before setting the `on_update`
        queue
            .queue_jobs(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                &temporary_directory,
            )
            .unwrap();

        // Then register the `on_update`
        queue.set_on_update(on_update);

        // Next, make sure updates roll in incrementally!
        let test_code = || {
            // `queue.queue_grouped_job()` -----------------------------------------------------------------------------

            queue
                .queue_grouped_job(
                    BASE_WORKFLOW,
                    SAMPLE_FILES,
                    PROTEIN_FASTA_FILE,
                    Some(MODIFICATIONS_FILE),
                    &temporary_directory,
                )
                .unwrap();

            assert_unpark_within_ms!(thread_parker, 75);
            assert!(thread_parker.no_missed_parks());

            // `queue.run()` -------------------------------------------------------------------------------------------

            queue.run().unwrap();

            assert_unpark_within_ms!(thread_parker, 1);
            assert_unpark_within_ms!(thread_parker, 5);
            assert_unpark_within_ms!(thread_parker, 15);
            assert_unpark_within_ms!(thread_parker, 20);
            assert_unpark_within_ms!(thread_parker, 25);
            assert!(thread_parker.no_missed_parks());

            // `queue.reset()` -----------------------------------------------------------------------------------------

            queue.reset().unwrap();

            assert_unpark_within_ms!(thread_parker, 1);
            assert_unpark_within_ms!(thread_parker, 5);
            assert_unpark_within_ms!(thread_parker, 5);
            assert_unpark_within_ms!(thread_parker, 5);
            assert_unpark_within_ms!(thread_parker, 15);
            assert!(thread_parker.no_missed_parks());

            // `queue.reset_job(0)` ------------------------------------------------------------------------------------

            queue.reset_job(0).unwrap();

            assert_unpark_within_ms!(thread_parker, 1);
            assert_unpark_within_ms!(thread_parker, 5);
            assert_unpark_within_ms!(thread_parker, 15);
            assert_unpark_within_ms!(thread_parker, 20);
            assert_unpark_within_ms!(thread_parker, 35);
            assert_unpark_within_ms!(thread_parker, 20);
            assert!(thread_parker.no_missed_parks());

            // `queue.cancel()` ----------------------------------------------------------------------------------------

            queue.cancel().unwrap();

            assert_unpark_within_ms!(thread_parker, 1);
            assert_unpark_within_ms!(thread_parker, 5);
            assert_unpark_within_ms!(thread_parker, 1);
            assert!(thread_parker.no_missed_parks());

            // `queue.run()` -------------------------------------------------------------------------------------------

            queue.run().unwrap();

            assert_unpark_within_ms!(thread_parker, 1);
            assert_unpark_within_ms!(thread_parker, 5);
            assert_unpark_within_ms!(thread_parker, 80);
            assert_unpark_within_ms!(thread_parker, 1);
            assert!(thread_parker.no_missed_parks());

            // `queue.reset()` -----------------------------------------------------------------------------------------

            queue.reset().unwrap();

            assert_unpark_within_ms!(thread_parker, 1);
            assert!(thread_parker.no_missed_parks());

            // `queue.clear()` -----------------------------------------------------------------------------------------

            queue.clear().unwrap();

            assert_unpark_within_ms!(thread_parker, 1);
            assert!(thread_parker.no_missed_parks());

            // `queue.queue_jobs()` ------------------------------------------------------------------------------------

            queue
                .queue_jobs(
                    BASE_WORKFLOW,
                    SAMPLE_FILES,
                    PROTEIN_FASTA_FILE,
                    Some(MODIFICATIONS_FILE),
                    &temporary_directory,
                )
                .unwrap();

            assert_unpark_within_ms!(thread_parker, 75);
            assert!(thread_parker.no_missed_parks());

            // `queue.run()` -------------------------------------------------------------------------------------------

            queue.run().unwrap();

            assert_unpark_within_ms!(thread_parker, 1);
            assert_unpark_within_ms!(thread_parker, 5);
            assert!(thread_parker.no_missed_parks());

            // `queue.remove_job(0)` -----------------------------------------------------------------------------------

            queue.remove_job(0).unwrap();

            assert_unpark_within_ms!(thread_parker, 1);
            assert!(thread_parker.no_missed_parks());

            // `queue.clear()` -----------------------------------------------------------------------------------------

            queue.clear().unwrap();

            assert_unpark_within_ms!(thread_parker, 1);
            assert_unpark_within_ms!(thread_parker, 5);

            sleep_ms(5);
            assert!(thread_parker.no_missed_parks());
        };

        unsafe { with_test_path(FAST_PATH, test_code) }

        let updates = updates.lock().unwrap();
        let expected: &[&[(&[JobStatusDiscriminant], Status)]] = &[
            // `queue.queue_grouped_job()`
            &[(&[Queued, Queued, Queued], Status::Ready)],
            // `queue.run()`
            &[(&[Queued, Queued, Queued], Status::Starting)],
            &[(&[Running, Queued, Queued], Status::Running)],
            &[(&[Running, Running, Queued], Status::Running)],
            &[(&[Running, Running, Running], Status::Running)],
            &[(&[Failed, Running, Running], Status::Running)],
            // `queue.reset()`
            &[(&[Queued, Stopping, Stopping], Status::Starting)],
            &[
                (&[Queued, Queued, Stopping], Status::Starting),
                (&[Queued, Stopping, Queued], Status::Starting),
                // NOTE: It's possible that both `Stopping` `Job`s finish at around the same time and, even if there
                // will still be two calls to `on_update`, the first of those calls could observe this state
                (&[Queued, Queued, Queued], Status::Starting),
            ],
            &[(&[Queued, Queued, Queued], Status::Starting)],
            &[(&[Running, Queued, Queued], Status::Running)],
            &[(&[Running, Running, Queued], Status::Running)],
            // `queue.reset_job(0)`
            &[(&[Stopping, Running, Queued], Status::Running)],
            &[(&[Queued, Running, Queued], Status::Running)],
            &[(&[Running, Running, Queued], Status::Running)],
            &[(&[Running, Running, Running], Status::Running)],
            &[(&[Failed, Running, Running], Status::Running)],
            &[(&[Failed, Running, Completed], Status::Running)],
            // `queue.cancel()`
            &[(&[Failed, Stopping, Completed], Status::Stopping)],
            &[(&[Failed, Queued, Completed], Status::Stopping)],
            &[(&[Failed, Queued, Completed], Status::Paused)],
            // `queue.run()`
            &[(&[Failed, Queued, Completed], Status::Starting)],
            &[(&[Failed, Running, Completed], Status::Running)],
            &[(&[Failed, Completed, Completed], Status::Stopping)],
            &[(&[Failed, Completed, Completed], Status::Finished)],
            // `queue.reset()`
            &[(&[Queued, Queued, Queued], Status::Ready)],
            // `queue.clear()`
            &[(&[], Status::Empty)],
            // `queue.queue_jobs()`
            &[(&[Queued, Queued], Status::Ready)],
            // `queue.run()`
            &[(&[Queued, Queued], Status::Starting)],
            &[(&[Running, Queued], Status::Running)],
            // `queue.remove_job(0)`
            &[(&[Queued], Status::Starting)],
            // `queue.clear()`
            &[(&[], Status::Stopping)],
            &[(&[], Status::Empty)],
        ];

        for (index, (update, &expected)) in updates
            .iter()
            .map(|(s, r)| (&s[..], *r))
            .zip(expected)
            .enumerate()
        {
            assert!(
                expected.contains(&update),
                "comparison failed at index {index}\n\
                expected {update:?} to be one of {expected:?}"
            );
        }
        assert_eq!(updates.len(), expected.len());
    }
}
