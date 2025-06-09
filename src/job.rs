// Standard Library Imports
use std::{
    sync::{Arc, Mutex, RwLock},
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
        self.on_update();

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
                self.on_update();

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
                self.on_update();
            }
        }
    }
}

// Private Helper Code =================================================================================================

pub(crate) type OnUpdate = Arc<RwLock<Option<OnUpdateCallback>>>;
pub(crate) type OnUpdateCallback = Arc<dyn Fn() + Send + Sync>;

impl Job {
    pub(crate) fn quiet_reset(&self) -> Result<()> {
        let prereset_status = self.status();
        *self.status.lock().unwrap() = Status::default();

        if let Status::Running(handle, _) = prereset_status {
            handle.kill()?;
        }

        Ok(())
    }

    fn on_update(&self) {
        if let Some(ref on_update) = *self.on_update.read().unwrap() {
            on_update();
        }
    }
}

// Unit Tests ==========================================================================================================

#[cfg(test)]
pub(crate) mod tests {
    use std::{sync::RwLock, thread};

    use tempfile::tempdir;

    use crate::{
        worker_pool::tests::sleep_ms,
        workflow::tests::{
            BASE_WORKFLOW, MODIFICATIONS_FILE, PROTEIN_FASTA_FILE, SAMPLE_FILES, result_file_in,
            wflw_file_in, with_test_path,
        },
    };

    use super::*;

    // TODO: On Windows, use `formatc!()` here to change the prefix between `tests/scripts/windows` and
    // `tests/scripts/unix` at compile time
    const COMPLETES_PATH: &str = "tests/scripts/job-completes";
    const FAILS_PATH: &str = "tests/scripts/job-fails";

    pub struct IntervalInstant(Instant);

    impl IntervalInstant {
        pub fn now() -> Self {
            Self(Instant::now())
        }

        // NOTE: The "resetting" of the `Instant` to `Instant::now()` means that `IntervalInstant.elapsed()` gives the
        // elapsed `Duration` since the `IntervalInstant`'s creation *or* the last call to `IntervalInstant.elapsed()`.
        // This means tests can make assertions about interval `Duration`s without repeatedly calling `Instant::now()`
        // or relying on brittle running totals of the elapsed time (which make it difficult to insert new steps into
        // the middle of an existing test)
        pub fn elapsed(&mut self) -> Duration {
            let result = self.0.elapsed();
            *self = Self::now();
            result
        }
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
        let job = Arc::new(Job::new(workflow, Arc::new(RwLock::new(None))));

        thread::spawn({
            let job = Arc::clone(&job);
            move || unsafe {
                with_test_path(COMPLETES_PATH, || job.run().unwrap());
            }
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
        thread::spawn({
            let job = Arc::clone(&job);
            move || unsafe {
                with_test_path(FAILS_PATH, || job.run().unwrap());
            }
        });

        sleep_ms(5);

        assert!(matches!(job.status(), Status::Running(..)));

        sleep_ms(20);

        assert!(matches!(job.status(), Status::Failed(..)));
        if let Status::Failed(report, run_time) = job.status() {
            assert_eq!(
                format!("{report:#}"),
                format!(
                    r#"command ["PMi-Byos-Console.exe", "--mode=create-project", "--input", "{}", "--output", "{}"] exited with code 42"#,
                    wflw_file.display(),
                    result_file.display()
                )
            );
            assert!(Duration::from_millis(10) < run_time && run_time < Duration::from_millis(20));
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
        let on_update = Arc::new(RwLock::new(None));
        let job = Arc::new(RwLock::new(Job::new(workflow, Arc::clone(&on_update))));

        let current_thread = thread::current();
        let updates = Arc::new(Mutex::new(Vec::new()));
        let on_update_callback = Arc::new({
            let updates = Arc::clone(&updates);
            let job = Arc::clone(&job);
            move || {
                updates.lock().unwrap().push(job.read().unwrap().status());
                current_thread.unpark();
            }
        });

        *on_update.write().unwrap() = Some(on_update_callback);

        // Start the `Job` and make sure the callback is being invoked!
        thread::spawn({
            let job = Arc::clone(&job);
            move || unsafe {
                with_test_path(COMPLETES_PATH, || job.read().unwrap().run().unwrap());
            }
        });

        let timeout = Duration::from_millis(100);
        let mut start = IntervalInstant::now();

        thread::park_timeout(timeout);
        assert!(start.elapsed() < Duration::from_millis(5));

        thread::park_timeout(timeout);
        assert!(start.elapsed() < Duration::from_millis(15));

        job.read().unwrap().reset().unwrap();

        thread::park_timeout(timeout);
        assert!(start.elapsed() < Duration::from_millis(5));

        assert!(matches!(
            updates.lock().unwrap()[..],
            [Status::Running(..), Status::Completed(..), Status::Queued]
        ));
    }
}
