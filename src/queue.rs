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
    job::{Job, Status},
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
    pub fn running(&self) -> bool {
        self.worker_pool.running()
    }

    #[must_use]
    pub fn jobs(&self) -> Vec<(String, Status)> {
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
                "tried to remove `Job` {index}, but there are only {jobs_len} `Job`s in the `Queue`"
            ));
        };
        drop(jobs_guard);

        // NOTE: This kills any `removed_job`s that were still running
        removed_job.quiet_reset()?;

        self.on_update.call();

        Ok(())
    }

    // Queue Control ---------------------------------------------------------------------------------------------------

    pub fn run(&self) -> Result<()> {
        self.stagger_timer.resume();

        let queued_jobs = self
            .jobs
            .read()
            .unwrap()
            .iter()
            .filter(|job| matches!(job.status(), Status::Queued))
            .count();
        let available_workers = self.worker_pool.available_workers();
        let new_workers = cmp::min(queued_jobs, available_workers);

        self.spawn_workers(new_workers)?;

        Ok(())
    }

    pub fn cancel(&self) -> Result<()> {
        self.stagger_timer.cancel();

        for job in self.jobs.read().unwrap().iter() {
            if let Status::Running(..) = job.status() {
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
            job.quiet_reset()?;
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
        if self.running() {
            return Err(eyre!("the `Queue` must be stopped to `{method_name}()`"));
        }

        Ok(())
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

    fn cancelled(&self) -> bool {
        self.stagger_timer.cancelled()
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
        if self.running() && !self.cancelled() {
            // NOTE: `WorkerPool.spawn()` will check if there is room in the pool for another worker, if there isn't,
            // then it will return an `Err`. We don't actually mind this outcome, so just discard the `Result` with a
            // `let _ = ...` binding.
            let _ = self.spawn_workers(1);
        }
    }

    fn worker(jobs: &Jobs, stagger_timer: &StaggerTimer) {
        let next_job_staggered = || {
            let Some(timer_guard) = stagger_timer.wait() else {
                // NOTE: If we *didn't* time out, then this `Queue` must have been cancelled — return `None` to break out
                // of the worker loop and shutdown the thread
                return None;
            };

            Self::next_job(jobs).map(|job| (job, timer_guard))
        };

        while let Some((job, timer_guard)) = next_job_staggered() {
            // SAFETY: The call to `next_job_staggered()` should only ever return `Status::Queued` `Job`s, so
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

    fn next_job(jobs: &Jobs) -> Option<Arc<Job>> {
        jobs.read()
            .unwrap()
            .iter()
            .find(|job| matches!(job.status(), Status::Queued))
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
        job::StatusDiscriminant::{self, *},
        worker_pool::tests::sleep_ms,
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

    fn job_statuses(queue: &Queue) -> Vec<StatusDiscriminant> {
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
        assert!(matches!(status, Status::Queued));

        let (name, status) = &jobs[1];
        assert_eq!(name, LDT_WORKFLOW_NAME);
        assert!(matches!(status, Status::Queued));
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
        assert!(matches!(status, Status::Queued));
    }

    #[test]
    fn run_checking_staggering() {
        let temporary_directory = tempdir().unwrap();

        // First, queue some `Job`s
        let queue = Queue::new(3, Duration::from_millis(10)).unwrap();

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

        assert!(!queue.running());

        let expected_job_names = [WT_WORKFLOW_NAME, LDT_WORKFLOW_NAME, WORKFLOW_NAME];
        let jobs = queue.jobs();
        let job_names: Vec<_> = jobs.iter().map(|(name, _)| name).collect();
        assert_eq!(job_names, expected_job_names);
        assert_eq!(job_statuses(&queue), [Queued, Queued, Queued]);

        // Then, make sure they run staggered and in parallel!
        let test_code = || {
            queue.run().unwrap();

            assert!(queue.running());

            sleep_ms(5);

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

            assert!(!queue.running());
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

        assert!(!queue.running());
        assert_eq!(queue.worker_pool.available_workers(), 2);
        assert_eq!(job_statuses(&queue), [Queued]);

        // Then, run the queue and add some more `Job`s whilst it's running!
        let test_code = || {
            queue.run().unwrap();

            assert!(queue.running());

            sleep_ms(5);

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

            assert!(!queue.running());
            assert_eq!(queue.worker_pool.available_workers(), 2);
            assert_eq!(job_statuses(&queue), [Completed, Completed, Completed]);
        };

        unsafe { with_test_path(SLOW_PATH, test_code) }
    }

    #[test]
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

        assert!(!queue.running());
        assert_eq!(queue.worker_pool.available_workers(), 1);
        assert_eq!(job_statuses(&queue), [Queued, Queued]);

        // Then, run the queue and make sure it cannot be reconfigured whilst running
        let test_code = || {
            queue.run().unwrap();

            assert!(queue.running());

            sleep_ms(5);

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

            sleep_ms(50);

            assert_eq!(queue.worker_pool.available_workers(), 1);
            assert_eq!(job_statuses(&queue), [Completed, Completed]);

            assert!(!queue.running());
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

        // Run the queue again, making sure that our changes were applied and that `Job` history is retained
        let test_code = || {
            queue.run().unwrap();

            assert!(queue.running());

            sleep_ms(10);

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

            assert!(!queue.running());
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
    fn stop() {
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

        assert!(!queue.running());
        assert_eq!(queue.worker_pool.available_workers(), 2);
        assert_eq!(job_statuses(&queue), [Queued, Queued, Queued]);

        // Then, run the queue and stop it whilst it's running!
        let test_code = || {
            queue.run().unwrap();

            assert!(queue.running());

            sleep_ms(5);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Running, Queued, Queued]);

            sleep_ms(25);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Running, Running, Queued]);

            sleep_ms(20);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Completed, Running, Running]);

            queue.cancel().unwrap();

            sleep_ms(5);

            assert!(!queue.running());
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

        assert!(!queue.running());
        assert_eq!(queue.worker_pool.available_workers(), 2);
        assert_eq!(job_statuses(&queue), [Queued, Queued, Queued]);

        // Then, make sure `Job`s can be removed whilst it's running
        let test_code = || {
            queue.run().unwrap();

            assert!(queue.running());

            sleep_ms(5);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Running, Queued, Queued]);

            queue.remove_job(1).unwrap();

            assert!(queue.running());
            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Running, Queued]);

            sleep_ms(15);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert_eq!(job_statuses(&queue), [Running, Running]);

            queue.remove_job(0).unwrap();

            assert!(queue.running());
            assert_eq!(job_statuses(&queue), [Running]);

            sleep_ms(5);

            assert_eq!(queue.worker_pool.available_workers(), 1);

            sleep_ms(50);

            assert_eq!(queue.worker_pool.available_workers(), 1);
            assert_eq!(job_statuses(&queue), [Running]);

            sleep_ms(15);

            assert_eq!(queue.worker_pool.available_workers(), 2);
            assert_eq!(job_statuses(&queue), [Completed]);

            assert!(!queue.running());

            queue.remove_job(0).unwrap();
            assert_eq!(job_statuses(&queue), []);

            assert_eq!(
                queue.remove_job(0).unwrap_err().to_string(),
                "tried to remove `Job` 0, but there are only 0 `Job`s in the `Queue`"
            );
        };

        unsafe { with_test_path(FAST_PATH, test_code) }
    }

    #[test]
    // NOTE: It's a test, so I think this is alright for now
    #[expect(clippy::too_many_lines)]
    // NOTE: Implementing the suggestions here lead to the test not compiling
    #[expect(clippy::significant_drop_tightening)]
    fn on_update_callback() {
        let temporary_directory = tempdir().unwrap();

        // First, set up a `Queue` with a debugging / logging callback
        let queue = Arc::new(Queue::new(3, Duration::from_millis(10)).unwrap());

        let current_thread = thread::current();
        let updates = Arc::new(Mutex::new(Vec::new()));
        let on_update = Arc::new({
            let updates = Arc::clone(&updates);
            let queue = Arc::clone(&queue);
            move || {
                updates
                    .lock()
                    .unwrap()
                    .push((job_statuses(&queue), queue.running()));
                current_thread.unpark();
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

            assert_unpark_within_ms!(75);

            // `queue.run()` -------------------------------------------------------------------------------------------

            queue.run().unwrap();

            assert_unpark_within_ms!(1);
            assert_unpark_within_ms!(5);
            assert_unpark_within_ms!(15);
            assert_unpark_within_ms!(15);
            assert_unpark_within_ms!(25);

            // `queue.reset()` -----------------------------------------------------------------------------------------

            queue.reset().unwrap();

            assert_unpark_within_ms!(1);
            assert_unpark_within_ms!(5);
            assert_unpark_within_ms!(15);
            assert_unpark_within_ms!(15);
            assert_unpark_within_ms!(25);
            assert_unpark_within_ms!(25);

            // `queue.cancel()` ----------------------------------------------------------------------------------------

            queue.cancel().unwrap();

            assert_unpark_within_ms!(1);
            assert_unpark_within_ms!(1);

            // `queue.reset()` -----------------------------------------------------------------------------------------

            queue.reset().unwrap();

            assert_unpark_within_ms!(1);

            // `queue.clear()` -----------------------------------------------------------------------------------------

            queue.clear().unwrap();

            assert_unpark_within_ms!(1);

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

            assert_unpark_within_ms!(75);

            // `queue.run()` -------------------------------------------------------------------------------------------

            queue.run().unwrap();

            assert_unpark_within_ms!(1);
            assert_unpark_within_ms!(5);

            // `queue.remove_job()` ------------------------------------------------------------------------------------

            queue.remove_job(0).unwrap();

            assert_unpark_within_ms!(1);

            // `queue.clear()` -----------------------------------------------------------------------------------------

            queue.clear().unwrap();

            assert_unpark_within_ms!(1);
            assert_unpark_within_ms!(5);
        };

        unsafe { with_test_path(FAST_PATH, test_code) }

        let updates = updates.lock().unwrap();
        let expected: &[(&[StatusDiscriminant], bool)] = &[
            // `queue.queue_grouped_job()`
            (&[Queued, Queued, Queued], false),
            // `queue.run()`
            (&[Queued, Queued, Queued], true),
            (&[Running, Queued, Queued], true),
            (&[Running, Running, Queued], true),
            (&[Running, Running, Running], true),
            (&[Failed, Running, Running], true),
            // `queue.reset()`
            (&[Queued, Queued, Queued], true),
            (&[Running, Queued, Queued], true),
            (&[Running, Running, Queued], true),
            (&[Running, Running, Running], true),
            (&[Failed, Running, Running], true),
            (&[Failed, Running, Completed], true),
            // `queue.cancel()`
            (&[Failed, Queued, Completed], true),
            (&[Failed, Queued, Completed], false),
            // `queue.reset()`
            (&[Queued, Queued, Queued], false),
            // `queue.clear()`
            (&[], false),
            // `queue.queue_jobs()`
            (&[Queued, Queued], false),
            // `queue.run()`
            (&[Queued, Queued], true),
            (&[Running, Queued], true),
            // `queue.remove_job()`
            (&[Queued], true),
            // `queue.clear()`
            (&[], true),
            (&[], false),
        ];

        for (index, (update, &expected)) in updates
            .iter()
            .map(|(s, r)| (&s[..], *r))
            .zip(expected)
            .enumerate()
        {
            assert_eq!(update, expected, "comparison failed at index {index}");
        }
        assert_eq!(updates.len(), expected.len());
    }
}
