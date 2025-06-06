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
    job::{Job, OnUpdate, OnUpdateCallback, Status},
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
        let jobs = Arc::new(RwLock::new(Vec::new()));
        let worker_pool = WorkerPool::new(workers)?;
        let stagger_timer = Self::stagger_timer(stagger_duration)?;
        let on_update = Arc::new(RwLock::new(None));

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

        self.worker_pool = WorkerPool::new(workers)?;

        Ok(())
    }

    pub fn set_stagger_duration(&mut self, stagger_duration: Duration) -> Result<()> {
        self.error_if_running("set_stagger_duration")?;

        self.stagger_timer = Self::stagger_timer(stagger_duration)?;

        Ok(())
    }

    pub fn set_on_update(&self, on_update: OnUpdateCallback) {
        *self.on_update.write().unwrap() = Some(on_update);
    }

    pub fn clear_on_update(&self) {
        *self.on_update.write().unwrap() = None;
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
            self.queue_grouped_job(
                base_workflow,
                &[sample_file],
                protein_file,
                modifications_file,
                output_directory,
            )?;
        }

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
        self.on_update();

        if self.running() && !self.cancelled() {
            // NOTE: `WorkerPool.spawn()` will check if there is room in the pool for another worker, if there isn't,
            // then it will return an `Err`. We don't actually mind this outcome, so just discard the `Result` with a
            // `let _ = ...` binding.
            let _ = self.spawn_workers(1);
        }

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

        if let Status::Running(handle, _) = removed_job.status() {
            // NOTE: Killing this running `Job` will result its status being set to `Failed(..)`, which will trigger
            // an `on_update()` call; consequentially, we don't need to call `self.on_update()` a second time in this
            // branch. Because this `Queue` and all of its jobs share a single `on_update` `Arc<Mutex<...>>`, `Job` will
            // always invoke the exact same callback as we would have by calling `self.on_update()`
            handle.kill()?;
        } else {
            self.on_update();
        }

        Ok(())
    }

    // Queue Control ---------------------------------------------------------------------------------------------------

    pub fn run(&self) -> Result<usize> {
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

        self.spawn_workers(new_workers)
    }

    pub fn cancel(&self) -> Result<()> {
        self.stagger_timer.cancel();

        for job in self.jobs.read().unwrap().iter() {
            if let Status::Running(..) = job.status() {
                job.reset()?;
            }
        }

        Ok(())
    }

    // TODO: Allow this to be run when the `Queue` is running. Make sure to start by cancelling the `StaggerTimer`!
    pub fn clear_jobs(&self) -> Result<()> {
        self.error_if_running("clear_jobs")?;

        self.jobs.write().unwrap().clear();
        self.on_update();

        Ok(())
    }

    // TODO: Allow this to be run when the `Queue` is running. Make sure to start by cancelling the `StaggerTimer`!
    pub fn reset_jobs(&self) -> Result<()> {
        self.error_if_running("reset_jobs")?;

        for job in self.jobs.read().unwrap().iter() {
            job.reset()?;
        }

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

    fn on_update(&self) {
        if let Some(ref on_update) = *self.on_update.read().unwrap() {
            on_update();
        }
    }

    fn cancelled(&self) -> bool {
        self.stagger_timer.cancelled()
    }

    fn spawn_workers(&self, new_workers: usize) -> Result<usize> {
        let jobs = Arc::clone(&self.jobs);
        let stagger_timer = Arc::clone(&self.stagger_timer);

        self.worker_pool.spawn(new_workers, move || {
            Self::worker(&jobs, &stagger_timer);
        })
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
    use std::{
        sync::{Mutex, RwLock},
        thread,
    };

    use tempfile::tempdir;

    use crate::{
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

    fn job_statuses(queue: &Queue) -> Vec<Status> {
        queue.jobs().into_iter().map(|(_, status)| status).collect()
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
        let all_jobs_are_queued = jobs
            .iter()
            .all(|(_, status)| matches!(status, Status::Queued));
        assert!(all_jobs_are_queued);

        // Then, make sure they run staggered and in parallel!
        let test_code = || {
            queue.run().unwrap();

            assert!(queue.running());

            sleep_ms(5);

            assert!(matches!(
                &job_statuses(&queue)[..],
                [Status::Running(..), Status::Queued, Status::Queued]
            ));

            sleep_ms(15);

            assert!(matches!(
                &job_statuses(&queue)[..],
                [Status::Running(..), Status::Running(..), Status::Queued]
            ));

            sleep_ms(15);

            assert!(matches!(
                &job_statuses(&queue)[..],
                [
                    Status::Running(..),
                    Status::Running(..),
                    Status::Running(..)
                ]
            ));

            sleep_ms(20);

            assert!(matches!(
                &job_statuses(&queue)[..],
                [Status::Failed(..), Status::Running(..), Status::Running(..)]
            ));

            sleep_ms(20);

            assert!(matches!(
                &job_statuses(&queue)[..],
                [
                    Status::Failed(..),
                    Status::Running(..),
                    Status::Completed(..)
                ]
            ));

            sleep_ms(20);

            assert!(matches!(
                &job_statuses(&queue)[..],
                [
                    Status::Failed(..),
                    Status::Completed(..),
                    Status::Completed(..)
                ]
            ));

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
        assert!(matches!(&job_statuses(&queue)[..], [Status::Queued]));

        // Then, run the queue and add some more `Job`s whilst it's running!
        let test_code = || {
            queue.run().unwrap();

            assert!(queue.running());

            sleep_ms(5);

            assert_eq!(queue.worker_pool.available_workers(), 1);
            assert!(matches!(&job_statuses(&queue)[..], [Status::Running(..)]));

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
            assert!(matches!(
                &job_statuses(&queue)[..],
                [Status::Running(..), Status::Running(..), Status::Queued]
            ));

            sleep_ms(40);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert!(matches!(
                &job_statuses(&queue)[..],
                [
                    Status::Completed(..),
                    Status::Running(..),
                    Status::Running(..)
                ]
            ));

            sleep_ms(170);

            assert_eq!(queue.worker_pool.available_workers(), 1);
            assert!(matches!(
                &job_statuses(&queue)[..],
                [
                    Status::Completed(..),
                    Status::Completed(..),
                    Status::Running(..)
                ]
            ));

            sleep_ms(40);

            assert!(!queue.running());
            assert_eq!(queue.worker_pool.available_workers(), 2);
            assert!(matches!(
                &job_statuses(&queue)[..],
                [
                    Status::Completed(..),
                    Status::Completed(..),
                    Status::Completed(..)
                ]
            ));
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

        assert!(!queue.running());
        assert_eq!(queue.worker_pool.available_workers(), 1);
        assert!(matches!(
            &job_statuses(&queue)[..],
            [Status::Queued, Status::Queued]
        ));

        // Then, run the queue and make sure it cannot be reconfigured whilst running
        let test_code = || {
            queue.run().unwrap();

            assert!(queue.running());

            sleep_ms(5);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert!(matches!(
                &job_statuses(&queue)[..],
                [Status::Running(..), Status::Queued]
            ));

            sleep_ms(50);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert!(matches!(
                &job_statuses(&queue)[..],
                [Status::Completed(..), Status::Queued]
            ));

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
            assert_eq!(
                queue.clear_jobs().unwrap_err().to_string(),
                "the `Queue` must be stopped to `clear_jobs()`"
            );
            assert_eq!(
                queue.reset_jobs().unwrap_err().to_string(),
                "the `Queue` must be stopped to `reset_jobs()`"
            );

            sleep_ms(20);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert!(matches!(
                &job_statuses(&queue)[..],
                [Status::Completed(..), Status::Running(..)]
            ));

            sleep_ms(50);

            assert_eq!(queue.worker_pool.available_workers(), 1);
            assert!(matches!(
                &job_statuses(&queue)[..],
                [Status::Completed(..), Status::Completed(..)]
            ));

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
            assert!(matches!(
                &job_statuses(&queue)[..],
                [
                    Status::Completed(..),
                    Status::Completed(..),
                    Status::Running(..),
                    Status::Running(..)
                ]
            ));

            sleep_ms(50);

            assert_eq!(queue.worker_pool.available_workers(), 2);
            assert!(matches!(
                &job_statuses(&queue)[..],
                [
                    Status::Completed(..),
                    Status::Completed(..),
                    Status::Failed(..),
                    Status::Running(..)
                ]
            ));

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
            assert_eq!(
                queue.clear_jobs().unwrap_err().to_string(),
                "the `Queue` must be stopped to `clear_jobs()`"
            );
            assert_eq!(
                queue.reset_jobs().unwrap_err().to_string(),
                "the `Queue` must be stopped to `reset_jobs()`"
            );

            sleep_ms(30);

            assert_eq!(queue.worker_pool.available_workers(), 3);
            assert!(matches!(
                &job_statuses(&queue)[..],
                [
                    Status::Completed(..),
                    Status::Completed(..),
                    Status::Failed(..),
                    Status::Completed(..)
                ]
            ));

            assert!(!queue.running());
        };

        unsafe { with_test_path(FAST_PATH, test_code) }

        assert!(matches!(
            &job_statuses(&queue)[..],
            [
                Status::Completed(..),
                Status::Completed(..),
                Status::Failed(..),
                Status::Completed(..)
            ]
        ));

        queue.reset_jobs().unwrap();

        assert!(matches!(
            &job_statuses(&queue)[..],
            [
                Status::Queued,
                Status::Queued,
                Status::Queued,
                Status::Queued
            ]
        ));

        queue.clear_jobs().unwrap();

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
        assert!(matches!(
            &job_statuses(&queue)[..],
            [Status::Queued, Status::Queued, Status::Queued]
        ));

        // Then, run the queue and stop it whilst it's running!
        let test_code = || {
            queue.run().unwrap();

            assert!(queue.running());

            sleep_ms(5);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert!(matches!(
                &job_statuses(&queue)[..],
                [Status::Running(..), Status::Queued, Status::Queued]
            ));

            sleep_ms(25);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert!(matches!(
                &job_statuses(&queue)[..],
                [Status::Running(..), Status::Running(..), Status::Queued]
            ));

            sleep_ms(20);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert!(matches!(
                &job_statuses(&queue)[..],
                [
                    Status::Completed(..),
                    Status::Running(..),
                    Status::Running(..)
                ]
            ));

            queue.cancel().unwrap();

            sleep_ms(5);

            assert!(!queue.running());
            assert!(queue.cancelled());
            assert_eq!(queue.worker_pool.available_workers(), 2);
            assert!(matches!(
                &job_statuses(&queue)[..],
                [Status::Completed(..), Status::Queued, Status::Queued]
            ));
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
        assert!(matches!(
            &job_statuses(&queue)[..],
            [Status::Queued, Status::Queued, Status::Queued]
        ));

        // Then, make sure `Job`s can be removed whilst it's running
        // NOTE: It's a test, so I'm okay with the "complexity" here
        #[expect(clippy::cognitive_complexity)]
        let test_code = || {
            queue.run().unwrap();

            assert!(queue.running());

            sleep_ms(5);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert!(matches!(
                &job_statuses(&queue)[..],
                [Status::Running(..), Status::Queued, Status::Queued]
            ));

            queue.remove_job(1).unwrap();

            assert!(queue.running());
            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert!(matches!(
                &job_statuses(&queue)[..],
                [Status::Running(..), Status::Queued]
            ));

            sleep_ms(15);

            assert_eq!(queue.worker_pool.available_workers(), 0);
            assert!(matches!(
                &job_statuses(&queue)[..],
                [Status::Running(..), Status::Running(..)]
            ));

            queue.remove_job(0).unwrap();

            assert!(queue.running());
            assert!(matches!(&job_statuses(&queue)[..], [Status::Running(..)]));

            sleep_ms(5);

            assert_eq!(queue.worker_pool.available_workers(), 1);

            sleep_ms(50);

            assert_eq!(queue.worker_pool.available_workers(), 1);
            assert!(matches!(&job_statuses(&queue)[..], [Status::Running(..)]));

            sleep_ms(15);

            assert_eq!(queue.worker_pool.available_workers(), 2);
            assert!(matches!(&job_statuses(&queue)[..], [Status::Completed(..)]));

            assert!(!queue.running());

            queue.remove_job(0).unwrap();
            assert!(matches!(&job_statuses(&queue)[..], []));

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
        let queue = Arc::new(RwLock::new(
            Queue::new(3, Duration::from_millis(10)).unwrap(),
        ));

        let current_thread = thread::current();
        let updates = Arc::new(Mutex::new(Vec::new()));
        let on_update = Arc::new({
            let updates = Arc::clone(&updates);
            let queue = Arc::clone(&queue);
            move || {
                updates
                    .lock()
                    .unwrap()
                    .push(job_statuses(&queue.read().unwrap()));
                current_thread.unpark();
            }
        });

        // Start by queueing a couple of `Job`s before setting the `on_update`
        queue
            .read()
            .unwrap()
            .queue_jobs(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                &temporary_directory,
            )
            .unwrap();

        // Then register the `on_update`
        queue.write().unwrap().set_on_update(on_update);
        let queue = queue.read().unwrap();

        let timeout = Duration::from_millis(300);
        let start = Instant::now();

        // And queue the rest of the `Job`s
        queue
            .queue_grouped_job(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                &temporary_directory,
            )
            .unwrap();

        thread::park_timeout(timeout);
        assert!(start.elapsed() < Duration::from_millis(75));

        assert!(!queue.running());

        // Next, make sure updates roll in incrementally!
        let test_code = || {
            queue.run().unwrap();

            assert!(queue.running());

            let start = Instant::now();

            thread::park_timeout(timeout);
            assert!(start.elapsed() < Duration::from_millis(5));

            thread::park_timeout(timeout);
            assert!(start.elapsed() < Duration::from_millis(20));

            thread::park_timeout(timeout);
            assert!(start.elapsed() < Duration::from_millis(35));

            thread::park_timeout(timeout);
            assert!(start.elapsed() < Duration::from_millis(55));

            thread::park_timeout(timeout);
            assert!(start.elapsed() < Duration::from_millis(75));

            queue.cancel().unwrap();

            thread::park_timeout(timeout);
            assert!(start.elapsed() < Duration::from_millis(80));

            sleep_ms(5);

            assert!(!queue.running());

            queue.reset_jobs().unwrap();

            thread::park_timeout(timeout);
            assert!(start.elapsed() < Duration::from_millis(90));

            queue.clear_jobs().unwrap();

            thread::park_timeout(timeout);
            assert!(start.elapsed() < Duration::from_millis(95));

            queue
                .queue_jobs(
                    BASE_WORKFLOW,
                    SAMPLE_FILES,
                    PROTEIN_FASTA_FILE,
                    Some(MODIFICATIONS_FILE),
                    &temporary_directory,
                )
                .unwrap();

            thread::park_timeout(timeout);
            assert!(start.elapsed() < Duration::from_millis(250));
        };

        unsafe { with_test_path(FAST_PATH, test_code) }

        let updates = updates.lock().unwrap();
        let updates: Vec<_> = updates.iter().map(Vec::as_slice).collect();
        assert!(matches!(
            &updates[..],
            [
                [Status::Queued, Status::Queued, Status::Queued],
                [Status::Running(..), Status::Queued, Status::Queued],
                [Status::Running(..), Status::Running(..), Status::Queued],
                [
                    Status::Running(..),
                    Status::Running(..),
                    Status::Running(..)
                ],
                [Status::Failed(..), Status::Running(..), Status::Running(..)],
                [
                    Status::Failed(..),
                    Status::Running(..),
                    Status::Completed(..)
                ],
                [Status::Failed(..), Status::Queued, Status::Completed(..)],
                [Status::Queued, Status::Queued, Status::Completed(..)],
                [Status::Queued, Status::Queued, Status::Queued],
                [],
                [Status::Queued],
                [Status::Queued, Status::Queued],
            ]
        ));
    }
}
