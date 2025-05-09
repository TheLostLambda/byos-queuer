// Standard Library Imports
use std::{
    cmp,
    path::Path,
    sync::{Arc, Mutex},
    thread,
    time::{Duration, Instant},
};

// External Crate Imports
use color_eyre::eyre::{OptionExt, Result, eyre};

// Local Crate Imports
use crate::{
    job::{Job, Status},
    worker_pool::WorkerPool,
    workflow::Workflow,
};

// Public API ==========================================================================================================

pub struct Queue {
    jobs: Jobs,
    worker_pool: WorkerPool,
    stagger_duration: Duration,
    last_job_run_at: SyncInstant,
}

impl Queue {
    pub fn new(workers: usize, stagger_duration: Duration) -> Result<Self> {
        let jobs = Arc::new(Mutex::new(Vec::new()));
        let worker_pool = WorkerPool::new(workers)?;
        // NOTE: By setting this `Instant` to be `stagger_duration` in the past, we're ensuring no launch delay for the
        // first job — the `stagger_duration` will have already "passed" by the time the `Queue` is constructed
        let last_job_run_at = Arc::new(Mutex::new(
            Instant::now()
                .checked_sub(stagger_duration)
                .ok_or_eyre("`stagger_duration` was too large")?,
        ));

        Ok(Self {
            jobs,
            worker_pool,
            stagger_duration,
            last_job_run_at,
        })
    }

    // FIXME: Needs testing!
    pub fn set_workers(&mut self, workers: usize) -> Result<()> {
        if self.running() {
            return Err(eyre!("the `Queue` must be stopped to `set_workers()`"));
        }

        self.worker_pool = WorkerPool::new(workers)?;

        Ok(())
    }

    // FIXME: Needs testing!
    pub fn set_stagger_duration(&mut self, stagger_duration: Duration) -> Result<()> {
        if self.running() {
            return Err(eyre!(
                "the `Queue` must be stopped to `set_stagger_duration()`"
            ));
        }

        self.stagger_duration = stagger_duration;

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

    // TODO: If the `Queue` is already running, then any new workflows added to the queue are started immediately!
    // Otherwise, just add them to `jobs` and wait for `Queue.run()` to be called! This variable should be `true`
    // as long as anything in the `jobs` list isn't finished (i.e. `Queued` or `Running`)
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

        self.jobs.lock().unwrap().push(Arc::new(Job::new(workflow)));

        Ok(())
    }

    pub fn run(&self) -> Result<usize> {
        let queued_jobs = self
            .jobs
            .lock()
            .unwrap()
            .iter()
            .filter(|job| matches!(job.status(), Status::Queued))
            .count();
        let available_workers = self.worker_pool.available_workers();
        let new_workers = cmp::min(queued_jobs, available_workers);

        let jobs = Arc::clone(&self.jobs);
        let stagger_duration = self.stagger_duration;
        let last_job_run_at = Arc::clone(&self.last_job_run_at);

        self.worker_pool.spawn(new_workers, move || {
            Self::worker(&jobs, stagger_duration, &last_job_run_at);
        })
    }
}

// Private Helper Code =================================================================================================

type Jobs = Arc<Mutex<Vec<Arc<Job>>>>;
type SyncInstant = Arc<Mutex<Instant>>;

impl Queue {
    fn running(&self) -> bool {
        self.worker_pool.running()
    }

    fn next_job(jobs: &Jobs) -> Option<Arc<Job>> {
        jobs.lock()
            .unwrap()
            .iter()
            .find(|job| matches!(job.status(), Status::Queued))
            .cloned()
    }

    fn worker(jobs: &Jobs, stagger_duration: Duration, last_job_run_at: &SyncInstant) {
        let next_job_staggered = || {
            let mut last_job_run_at = last_job_run_at.lock().unwrap();

            Self::sleep_until_elapsed(*last_job_run_at, stagger_duration);

            // NOTE: If the `jobs` `Mutex` is contended, then this might take some time. Keep the `last_job_run_at`
            // `Mutex` locked and only start the timer after `Self::next_job()` returns
            let next_job = Self::next_job(jobs);
            *last_job_run_at = Instant::now();

            next_job
        };

        while let Some(job) = next_job_staggered() {
            // SAFETY: The call to `next_job_staggered()` should only ever return `Status::Queued` `Job`s, so
            // `Job.run()` shouldn't ever fail!
            job.run().unwrap();
        }
    }

    fn sleep_until_elapsed(instant: Instant, target: Duration) {
        let elapsed = instant.elapsed();
        let duration_to_go = target.saturating_sub(elapsed);

        thread::sleep(duration_to_go);
    }
}

// Unit Tests ==========================================================================================================

#[cfg(test)]
mod tests {
    use std::fs;

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

    const TEST_PATH: &str = "tests/scripts/queue";

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
        let job_statuses = || {
            queue
                .jobs()
                .into_iter()
                .map(|(_, status)| status)
                .collect::<Vec<_>>()
        };
        let test_code = || {
            queue.run().unwrap();

            assert!(queue.running());

            sleep_ms(5);

            assert!(matches!(
                &job_statuses()[..],
                [Status::Running(..), Status::Queued, Status::Queued]
            ));

            sleep_ms(15);

            assert!(matches!(
                &job_statuses()[..],
                [Status::Running(..), Status::Running(..), Status::Queued]
            ));

            sleep_ms(15);

            assert!(matches!(
                &job_statuses()[..],
                [
                    Status::Running(..),
                    Status::Running(..),
                    Status::Running(..)
                ]
            ));

            sleep_ms(20);

            assert!(matches!(
                &job_statuses()[..],
                [Status::Failed(..), Status::Running(..), Status::Running(..)]
            ));

            sleep_ms(20);

            assert!(matches!(
                &job_statuses()[..],
                [
                    Status::Failed(..),
                    Status::Running(..),
                    Status::Completed(..)
                ]
            ));

            sleep_ms(20);

            assert!(matches!(
                &job_statuses()[..],
                [
                    Status::Failed(..),
                    Status::Completed(..),
                    Status::Completed(..)
                ]
            ));

            assert!(!queue.running());
        };

        unsafe { with_test_path(TEST_PATH, test_code) }

        fs::remove_dir_all(WT_RESULT_DIRECTORY).unwrap();
        fs::remove_dir_all(LDT_RESULT_DIRECTORY).unwrap();
        fs::remove_dir_all(RESULT_DIRECTORY).unwrap();
    }

    #[ignore]
    #[test]
    #[serial]
    fn run_checking_worker_count() {
        todo!()
    }
}
