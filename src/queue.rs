// Standard Library Imports
use std::{
    cmp,
    path::Path,
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

// External Crate Imports
use color_eyre::eyre::{OptionExt, Result, eyre};

// Local Crate Imports
use crate::{
    cancellable_timer::CancellableTimer,
    job::{Job, Status},
    worker_pool::WorkerPool,
    workflow::Workflow,
};

// Public API ==========================================================================================================

pub struct Queue {
    jobs: Jobs,
    worker_pool: WorkerPool,
    stagger_timer: StaggerTimer,
}

impl Queue {
    pub fn new(workers: usize, stagger_duration: Duration) -> Result<Self> {
        let jobs = Arc::new(Mutex::new(Vec::new()));
        let worker_pool = WorkerPool::new(workers)?;
        let stagger_timer = Self::stagger_timer(stagger_duration)?;

        Ok(Self {
            jobs,
            worker_pool,
            stagger_timer,
        })
    }

    pub fn set_workers(&mut self, workers: usize) -> Result<()> {
        if self.running() {
            return Err(eyre!("the `Queue` must be stopped to `set_workers()`"));
        }

        self.worker_pool = WorkerPool::new(workers)?;

        Ok(())
    }

    pub fn set_stagger_duration(&mut self, stagger_duration: Duration) -> Result<()> {
        if self.running() {
            return Err(eyre!(
                "the `Queue` must be stopped to `set_stagger_duration()`"
            ));
        }

        self.stagger_timer = Self::stagger_timer(stagger_duration)?;

        Ok(())
    }

    #[must_use]
    pub fn jobs(&self) -> Vec<(String, Status)> {
        self.jobs
            .lock()
            .unwrap()
            .iter()
            .map(|job| (job.name().to_owned(), job.status()))
            .collect()
    }

    pub fn queue(
        &self,
        base_workflow: impl AsRef<Path> + Copy,
        sample_files: impl IntoIterator<Item = impl AsRef<Path>> + Copy,
        protein_file: impl AsRef<Path> + Copy,
        modifications_file: Option<impl AsRef<Path> + Copy>,
        output_directory: impl AsRef<Path> + Copy,
    ) -> Result<()> {
        for sample_file in sample_files {
            self.queue_grouped(
                base_workflow,
                &[sample_file],
                protein_file,
                modifications_file,
                output_directory,
            )?;
        }

        Ok(())
    }

    pub fn queue_grouped(
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

        // FIXME: Put something real as the second argument here!
        self.jobs
            .lock()
            .unwrap()
            .push(Arc::new(Job::new(workflow, None)));

        if self.running() && !self.cancelled() {
            // NOTE: `WorkerPool.spawn()` will check if there is room in the pool for another worker, if there isn't,
            // then it will return an `Err`. We don't actually mind this outcome, so just discard the `Result` with a
            // `let _ = ...` binding.
            let _ = self.spawn_workers(1);
        }

        Ok(())
    }

    pub fn run(&self) -> Result<usize> {
        self.stagger_timer.resume();

        let queued_jobs = self
            .jobs
            .lock()
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

        for job in self.jobs.lock().unwrap().iter() {
            if let Status::Running(..) = job.status() {
                job.reset()?;
            }
        }

        Ok(())
    }
}

// Private Helper Code =================================================================================================

type Jobs = Arc<Mutex<Vec<Arc<Job>>>>;
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

    fn running(&self) -> bool {
        self.worker_pool.running()
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
        // NOTE: Keep an eye on https://github.com/rust-lang/rust-clippy/issues/12128, this is a false positive!
        #[allow(clippy::significant_drop_tightening)]
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
        jobs.lock()
            .unwrap()
            .iter()
            .find(|job| matches!(job.status(), Status::Queued))
            .cloned()
    }
}

// Unit Tests ==========================================================================================================

#[cfg(test)]
mod tests {
    use std::{fs, thread};

    use const_format::formatc;
    use serial_test::serial;

    use crate::{
        worker_pool::tests::sleep_ms,
        workflow::tests::{
            BASE_WORKFLOW, MODIFICATIONS_FILE, OUTPUT_DIRECTORY, PROTEIN_FASTA_FILE,
            RESULT_DIRECTORY, SAMPLE_FILES, WFLW_FILE, WORKFLOW_NAME, with_test_path,
        },
    };

    use super::*;

    const WT_WORKFLOW_NAME: &str = "PG Monomers (WT; proteins.fasta; modifications.txt)";
    const LDT_WORKFLOW_NAME: &str = "PG Monomers (6ldt; proteins.fasta; modifications.txt)";

    const WT_RESULT_DIRECTORY: &str = formatc!("{OUTPUT_DIRECTORY}/{WT_WORKFLOW_NAME}");
    const LDT_RESULT_DIRECTORY: &str = formatc!("{OUTPUT_DIRECTORY}/{LDT_WORKFLOW_NAME}");

    const WT_WFLW_FILE: &str = formatc!("{WT_RESULT_DIRECTORY}.wflw");
    const LDT_WFLW_FILE: &str = formatc!("{LDT_RESULT_DIRECTORY}.wflw");

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
    #[serial]
    fn queue() {
        let queue = Queue::new(1, Duration::default()).unwrap();

        queue
            .queue(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                OUTPUT_DIRECTORY,
            )
            .unwrap();

        let jobs = queue.jobs();

        let (name, status) = &jobs[0];
        assert_eq!(name, WT_WORKFLOW_NAME);
        assert!(matches!(status, Status::Queued));

        let (name, status) = &jobs[1];
        assert_eq!(name, LDT_WORKFLOW_NAME);
        assert!(matches!(status, Status::Queued));

        fs::remove_file(WT_WFLW_FILE).unwrap();
        fs::remove_file(LDT_WFLW_FILE).unwrap();
    }

    #[test]
    #[serial]
    fn queue_grouped() {
        let queue = Queue::new(1, Duration::default()).unwrap();

        queue
            .queue_grouped(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                OUTPUT_DIRECTORY,
            )
            .unwrap();

        let jobs = queue.jobs();

        let (name, status) = &jobs[0];
        assert_eq!(name, WORKFLOW_NAME);
        assert!(matches!(status, Status::Queued));

        fs::remove_file(WFLW_FILE).unwrap();
    }

    #[test]
    #[serial]
    fn run_checking_staggering() {
        // First, queue some `Job`s
        let queue = Queue::new(3, Duration::from_millis(10)).unwrap();

        queue
            .queue(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                OUTPUT_DIRECTORY,
            )
            .unwrap();

        queue
            .queue_grouped(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                OUTPUT_DIRECTORY,
            )
            .unwrap();

        fs::remove_file(WT_WFLW_FILE).unwrap();
        fs::remove_file(LDT_WFLW_FILE).unwrap();
        fs::remove_file(WFLW_FILE).unwrap();

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

        fs::remove_dir_all(WT_RESULT_DIRECTORY).unwrap();
        fs::remove_dir_all(LDT_RESULT_DIRECTORY).unwrap();
        fs::remove_dir_all(RESULT_DIRECTORY).unwrap();
    }

    #[test]
    #[serial]
    fn run_checking_addition_to_running_queue() {
        // First, queue an initial `Job`
        let queue = Queue::new(2, Duration::from_millis(100)).unwrap();

        queue
            .queue_grouped(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                OUTPUT_DIRECTORY,
            )
            .unwrap();

        fs::remove_file(WFLW_FILE).unwrap();

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
                .queue(
                    BASE_WORKFLOW,
                    SAMPLE_FILES,
                    PROTEIN_FASTA_FILE,
                    Some(MODIFICATIONS_FILE),
                    OUTPUT_DIRECTORY,
                )
                .unwrap();

            fs::remove_file(WT_WFLW_FILE).unwrap();
            fs::remove_file(LDT_WFLW_FILE).unwrap();

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

        fs::remove_dir_all(WT_RESULT_DIRECTORY).unwrap();
        fs::remove_dir_all(LDT_RESULT_DIRECTORY).unwrap();
        fs::remove_dir_all(RESULT_DIRECTORY).unwrap();
    }

    #[test]
    #[serial]
    fn reconfigure_when_stopped() {
        // First, queue an initial couple of `Job`s
        let mut queue = Queue::new(1, Duration::from_millis(60)).unwrap();

        for _ in 0..2 {
            queue
                .queue_grouped(
                    BASE_WORKFLOW,
                    SAMPLE_FILES,
                    PROTEIN_FASTA_FILE,
                    Some(MODIFICATIONS_FILE),
                    OUTPUT_DIRECTORY,
                )
                .unwrap();
        }

        fs::remove_file(WFLW_FILE).unwrap();

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
            .queue(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                OUTPUT_DIRECTORY,
            )
            .unwrap();

        fs::remove_file(WT_WFLW_FILE).unwrap();
        fs::remove_file(LDT_WFLW_FILE).unwrap();

        // Run the queue again, making sure that our changes were applied and that `Job` history is retained
        let test_code = || {
            queue.run().unwrap();

            assert!(queue.running());

            sleep_ms(5);

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

        fs::remove_dir_all(RESULT_DIRECTORY).unwrap();
        fs::remove_dir_all(WT_RESULT_DIRECTORY).unwrap();
        fs::remove_dir_all(LDT_RESULT_DIRECTORY).unwrap();
    }

    #[test]
    #[serial]
    fn stop() {
        // First, queue some initial `Job`s
        let queue = Queue::new(2, Duration::from_millis(20)).unwrap();

        queue
            .queue_grouped(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                OUTPUT_DIRECTORY,
            )
            .unwrap();

        queue
            .queue(
                BASE_WORKFLOW,
                SAMPLE_FILES,
                PROTEIN_FASTA_FILE,
                Some(MODIFICATIONS_FILE),
                OUTPUT_DIRECTORY,
            )
            .unwrap();

        fs::remove_file(WFLW_FILE).unwrap();
        fs::remove_file(WT_WFLW_FILE).unwrap();
        fs::remove_file(LDT_WFLW_FILE).unwrap();

        assert!(!queue.running());
        assert_eq!(queue.worker_pool.available_workers(), 2);
        assert!(matches!(
            &job_statuses(&queue)[..],
            [Status::Queued, Status::Queued, Status::Queued]
        ));

        // Then, run the queue and add some more `Job`s whilst it's running!
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

        fs::remove_dir_all(RESULT_DIRECTORY).unwrap();
        fs::remove_dir_all(WT_RESULT_DIRECTORY).unwrap();
        fs::remove_dir_all(LDT_RESULT_DIRECTORY).unwrap();
    }
}
