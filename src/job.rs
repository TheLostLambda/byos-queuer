// Standard Library Imports
use std::{
    sync::{Arc, Mutex},
    time::{Duration, Instant},
};

// External Crate Imports
use color_eyre::{
    Result,
    eyre::{Report, eyre},
};
use duct::Handle;

// Local Crate Imports
use crate::workflow::Workflow;

// Public API ==========================================================================================================

#[derive(Clone, Debug, Default)]
pub enum Status {
    #[default]
    Queued,
    Running(Arc<Handle>, Instant),
    Completed(Duration),
    Failed(Arc<Report>, Duration),
}

pub struct Job {
    workflow: Workflow,
    status: Mutex<Status>,
}

impl Job {
    #[must_use]
    pub fn new(workflow: Workflow) -> Self {
        let status = Mutex::new(Status::default());
        Self { workflow, status }
    }

    pub fn name(&self) -> &str {
        self.workflow.name()
    }

    #[must_use]
    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    pub fn reset(&self) -> Result<()> {
        if let Status::Running(handle, _) = self.status() {
            handle.kill()?;
        }

        *self.status.lock().unwrap() = Status::default();

        Ok(())
    }

    pub fn run(&self) -> Result<()> {
        self.start()?;
        self.wait();

        Ok(())
    }

    pub fn start(&self) -> Result<()> {
        match self.status() {
            Status::Queued => {
                let instant = Instant::now();
                let handle = Arc::new(self.workflow.start()?);
                *self.status.lock().unwrap() = Status::Running(Arc::clone(&handle), instant);

                Ok(())
            }
            Status::Running(..) => Err(eyre!("this `Job` is already running")),
            Status::Completed(..) | Status::Failed(..) => Err(eyre!(
                "the `Job` has already been run — if you'd like to re-run it, first reset its status with `Job.reset()`"
            )),
        }
    }

    pub fn wait(&self) {
        if let Status::Running(handle, instant) = self.status() {
            let exit_status = match handle.wait() {
                Ok(_) => Status::Completed(instant.elapsed()),
                Err(error) => Status::Failed(Arc::new(error.into()), instant.elapsed()),
            };

            // NOTE: We're rechecking the status to make sure it hasn't been reset by another thread since we started
            // waiting — if it has, we should leave the status as is!
            if let Status::Running(..) = self.status() {
                *self.status.lock().unwrap() = exit_status;
            }
        }
    }
}

// Unit Tests ==========================================================================================================

#[cfg(test)]
mod tests {
    use std::{fs, thread};

    use serial_test::serial;

    use crate::{
        worker_pool::tests::sleep_ms,
        workflow::tests::{
            BASE_WORKFLOW, MODIFICATIONS_FILE, OUTPUT_DIRECTORY, PROTEIN_FASTA_FILE,
            RESULT_DIRECTORY, SAMPLE_FILES, WFLW_FILE, with_test_path,
        },
    };

    use super::*;

    // TODO: On Windows, use `formatc!()` here to change the prefix between `tests/scripts/windows` and
    // `tests/scripts/unix` at compile time
    const COMPLETES_PATH: &str = "tests/scripts/job-completes";
    const FAILS_PATH: &str = "tests/scripts/job-fails";

    #[test]
    #[serial]
    fn new_then_run() {
        // Construct a new workflow
        let _ = fs::remove_file(WFLW_FILE);

        let workflow = Workflow::new(
            BASE_WORKFLOW,
            SAMPLE_FILES,
            PROTEIN_FASTA_FILE,
            Some(MODIFICATIONS_FILE),
            OUTPUT_DIRECTORY,
        )
        .unwrap();

        fs::remove_file(WFLW_FILE).unwrap();

        // Run a job that should complete sucessfully
        let job = Arc::new(Job::new(workflow));

        let completes_job = Arc::clone(&job);
        thread::spawn(move || unsafe {
            with_test_path(COMPLETES_PATH, || completes_job.run().unwrap());
        });

        sleep_ms(5);

        assert!(matches!(job.status(), Status::Running(..)));

        sleep_ms(20);

        assert!(matches!(job.status(), Status::Completed(..)));
        if let Status::Completed(run_time) = job.status() {
            assert!(Duration::from_millis(10) < run_time && run_time < Duration::from_millis(20));
        }

        job.reset().unwrap();

        // Run a job that should fail
        let fails_job = Arc::clone(&job);
        thread::spawn(move || unsafe { with_test_path(FAILS_PATH, || fails_job.run().unwrap()) });

        sleep_ms(5);

        assert!(matches!(job.status(), Status::Running(..)));

        sleep_ms(20);

        assert!(matches!(job.status(), Status::Failed(..)));
        if let Status::Failed(report, run_time) = job.status() {
            assert_eq!(
                format!("{report:#}"),
                r#"command ["PMi-Byos-Console.exe", "--mode=create-project", "--input", "tests/data/output/PG Monomers (WT, 6ldt; proteins.fasta; modifications.txt).wflw", "--output", "/home/tll/Documents/University/PhD/Scripts/Rust/byos-queuer/tests/data/output/PG Monomers (WT, 6ldt; proteins.fasta; modifications.txt)/Result"] exited with code 42"#
            );
            assert!(Duration::from_millis(10) < run_time && run_time < Duration::from_millis(20));
        }

        fs::remove_dir_all(RESULT_DIRECTORY).unwrap();
    }
}
