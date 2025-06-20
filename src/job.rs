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
use crate::{on_update::OnUpdate, workflow::Workflow};

// Public API ==========================================================================================================

#[derive(Clone, Debug, Default)]
pub enum Status {
    #[default]
    Queued,
    Running(Arc<Handle>, Instant),
    Resetting,
    Completed(Duration),
    Failed(Arc<Report>, Duration),
    Abandoned,
}

pub struct Job {
    workflow: Workflow,
    status: Mutex<Status>,
    on_update: OnUpdate,
}

impl Job {
    #[must_use]
    pub fn new(workflow: Workflow, on_update: OnUpdate) -> Self {
        let status = Mutex::new(Status::default());

        Self {
            workflow,
            status,
            on_update,
        }
    }

    pub fn name(&self) -> &str {
        self.workflow.name()
    }

    #[must_use]
    pub fn status(&self) -> Status {
        self.status.lock().unwrap().clone()
    }

    pub fn reset(&self) -> Result<()> {
        if matches!(self.status(), Status::Queued) {
            return Ok(());
        }

        self.quiet_reset()?;

        // NOTE: If we've made it here, we can be certain that `status` has changed and that we need to signal an
        // update — if the `Job` was already `Queued`, then we would have returned early!
        self.on_update.call();

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
                let handle = Arc::new(self.workflow.start()?);
                *self.status.lock().unwrap() = Status::Running(handle, Instant::now());
                self.on_update.call();

                Ok(())
            }
            Status::Running(..) => Err(eyre!("this `Job` is already running")),
            Status::Resetting => Err(eyre!(
                "this `Job` is resetting; please wait for it to fully stop before re-running"
            )),
            Status::Completed(..) | Status::Failed(..) => Err(eyre!(
                "the `Job` has already been run — if you'd like to re-run it, first reset its status with `Job.reset()`"
            )),
            Status::Abandoned => Err(eyre!("this `Job` has been abandoned")),
        }
    }

    pub fn wait(&self) {
        if let Status::Running(handle, instant) = self.status() {
            let exit_status = match handle.wait() {
                Ok(_) => Status::Completed(instant.elapsed()),
                Err(error) => Status::Failed(Arc::new(error.into()), instant.elapsed()),
            };

            // NOTE: We're rechecking the status in case another thread has changed it whilst we've been `.wait()`ing
            *self.status.lock().unwrap() = match self.status() {
                Status::Running(..) => exit_status,
                // NOTE: It takes some time to kill a `Running` `Job` after calling `Job.reset()` — with the child
                // process now fully dead, we can re-queue the `Job`
                Status::Resetting => Status::Queued,
                // NOTE: `Abandoned` `Job`s should not have their statuses updated and should not call `on_update` —
                // return early to avoid the `self.on_update.call()`
                Status::Abandoned => return,
                _ => unreachable!(),
            };

            self.on_update.call();
        }
    }
}

// Private Helper Code =================================================================================================

// NOTE: This is primarily useful for test code where I don't care about any of the `Status` fields, but I do need a
// `PartialEq` implementation to use `assert_eq!(...)`
#[derive(Clone, Eq, PartialEq, Debug)]
pub(crate) enum StatusDiscriminant {
    Queued,
    Running,
    Stopping,
    Completed,
    Failed,
    Abandoned,
}

impl From<Status> for StatusDiscriminant {
    fn from(value: Status) -> Self {
        match value {
            Status::Queued => Self::Queued,
            Status::Running(..) => Self::Running,
            Status::Resetting => Self::Stopping,
            Status::Completed(..) => Self::Completed,
            Status::Failed(..) => Self::Failed,
            Status::Abandoned => Self::Abandoned,
        }
    }
}

impl Job {
    pub(crate) fn quiet_reset(&self) -> Result<()> {
        if let Status::Running(handle, _) = self.status() {
            *self.status.lock().unwrap() = Status::Resetting;
            handle.kill()?;
        } else {
            *self.status.lock().unwrap() = Status::default();
        }

        Ok(())
    }

    pub(crate) fn abandon(&self) -> Result<()> {
        let prereset_status = self.status();
        *self.status.lock().unwrap() = Status::Abandoned;

        if let Status::Running(handle, _) = prereset_status {
            handle.kill()?;
        }

        Ok(())
    }
}

// Unit Tests ==========================================================================================================

#[cfg(test)]
pub(crate) mod tests {
    use std::thread;

    use tempfile::tempdir;

    use crate::{
        assert_unpark_within_ms,
        worker_pool::tests::{ThreadParker, sleep_ms},
        workflow::tests::{
            BASE_WORKFLOW, MODIFICATIONS_FILE, PROTEIN_FASTA_FILE, SAMPLE_FILES, result_file_in,
            wflw_file_in, with_test_path,
        },
    };

    use super::*;
    use StatusDiscriminant::*;

    // TODO: On Windows, use `formatc!()` here to change the prefix between `tests/scripts/windows` and
    // `tests/scripts/unix` at compile time
    const COMPLETES_PATH: &str = "tests/scripts/job-completes";
    const FAILS_PATH: &str = "tests/scripts/job-fails";

    fn status(job: &Job) -> StatusDiscriminant {
        StatusDiscriminant::from(job.status())
    }

    #[test]
    fn new_then_run() {
        let temporary_directory = tempdir().unwrap();
        let wflw_file = wflw_file_in(&temporary_directory);
        let result_file = result_file_in(&temporary_directory);

        // Construct a new workflow
        let workflow = Workflow::new(
            BASE_WORKFLOW,
            SAMPLE_FILES,
            PROTEIN_FASTA_FILE,
            Some(MODIFICATIONS_FILE),
            &temporary_directory,
        )
        .unwrap();

        // Run a job that should complete sucessfully
        let job = Arc::new(Job::new(workflow, OnUpdate::new()));

        thread::spawn({
            let job = Arc::clone(&job);
            move || unsafe {
                with_test_path(COMPLETES_PATH, || job.run().unwrap());
            }
        });

        sleep_ms(15);

        assert_eq!(status(&job), Running);

        sleep_ms(40);

        assert_eq!(status(&job), Completed);
        if let Status::Completed(run_time) = job.status() {
            assert!(Duration::from_millis(20) < run_time && run_time < Duration::from_millis(30));
        }

        job.reset().unwrap();

        // Run a job that should fail
        thread::spawn({
            let job = Arc::clone(&job);
            move || unsafe {
                with_test_path(FAILS_PATH, || job.run().unwrap());
            }
        });

        sleep_ms(15);

        assert_eq!(status(&job), Running);

        sleep_ms(40);

        assert_eq!(status(&job), Failed);
        if let Status::Failed(report, run_time) = job.status() {
            assert_eq!(
                format!("{report:#}"),
                format!(
                    r#"command ["PMi-Byos-Console.exe", "--mode=create-project", "--input", "{}", "--output", "{}"] exited with code 42"#,
                    wflw_file.display(),
                    result_file.display()
                )
            );
            assert!(Duration::from_millis(20) < run_time && run_time < Duration::from_millis(30));
        }
    }

    #[test]
    fn on_update_callback() {
        let temporary_directory = tempdir().unwrap();

        let workflow = Workflow::new(
            BASE_WORKFLOW,
            SAMPLE_FILES,
            PROTEIN_FASTA_FILE,
            Some(MODIFICATIONS_FILE),
            &temporary_directory,
        )
        .unwrap();

        // Construct a `Job` with a callback
        let on_update = OnUpdate::new();
        let job = Arc::new(Job::new(workflow, on_update.clone()));

        let thread_parker = Arc::new(ThreadParker::new());
        let updates = Arc::new(Mutex::new(Vec::new()));
        let on_update_callback = Arc::new({
            let updates = Arc::clone(&updates);
            let job = Arc::clone(&job);
            let thread_parker = Arc::clone(&thread_parker);
            move || {
                updates.lock().unwrap().push(status(&job));
                thread_parker.unpark();
            }
        });

        on_update.set(on_update_callback);

        // Start the `Job` and make sure the callback is being invoked!
        thread::spawn({
            let job = Arc::clone(&job);
            move || unsafe {
                with_test_path(COMPLETES_PATH, || job.run().unwrap());
            }
        });

        assert_unpark_within_ms!(thread_parker, 15);
        assert_unpark_within_ms!(thread_parker, 30);

        job.reset().unwrap();

        assert_unpark_within_ms!(thread_parker, 1);

        assert_eq!(updates.lock().unwrap()[..], [Running, Completed, Queued]);

        assert!(thread_parker.no_missed_parks());
    }
}
