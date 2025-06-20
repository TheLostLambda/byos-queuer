use std::process::Output;

use color_eyre::Result;

pub struct Handle {
    inner: duct::Handle,
    post_wait: Option<Hook>,
    post_kill: Option<Hook>,
}

type Hook = Box<dyn Fn() -> Result<()> + Send + Sync>;

impl Handle {
    pub fn set_post_wait(&mut self, post_wait: impl Fn() -> Result<()> + Send + Sync + 'static) {
        self.post_wait = Some(Box::new(post_wait) as Hook);
    }

    pub fn set_post_kill(&mut self, post_kill: impl Fn() -> Result<()> + Send + Sync + 'static) {
        self.post_kill = Some(Box::new(post_kill) as Hook);
    }

    pub fn wait(&self) -> Result<&Output> {
        let result = self.inner.wait();

        if let Some(post_wait) = &self.post_wait {
            post_wait()?;
        }

        result.map_err(Into::into)
    }

    pub fn kill(&self) -> Result<()> {
        let result = self.inner.kill();

        if let Some(post_kill) = &self.post_kill {
            post_kill()?;
        }

        result.map_err(Into::into)
    }
}

impl From<duct::Handle> for Handle {
    fn from(value: duct::Handle) -> Self {
        Self {
            inner: value,
            post_wait: None,
            post_kill: None,
        }
    }
}

#[cfg(test)]
mod tests {
    use std::{
        sync::{Arc, Mutex},
        time::{Duration, Instant},
    };

    use duct::cmd;

    use super::*;

    fn sleeping_handle() -> Handle {
        // TODO: Use conditional compilation to adapt this command for Windows
        cmd!("sh", "-c", "sleep 0.02 && echo 'Goodbye, World!'")
            .stdout_capture()
            .stderr_capture()
            .start()
            .unwrap()
            .into()
    }

    #[test]
    fn wait() {
        let start = Instant::now();
        let handle = sleeping_handle();
        let output = handle.wait().unwrap();
        let elapsed = start.elapsed();

        assert_eq!(output.status.code(), Some(0));
        assert_eq!(output.stdout, b"Goodbye, World!\n");
        assert_eq!(output.stderr, b"");
        assert!(elapsed > Duration::from_millis(20));
        assert!(elapsed < Duration::from_millis(25));
    }

    #[test]
    fn kill() {
        let handle = sleeping_handle();
        handle.kill().unwrap();
        let output = handle.wait();

        assert_eq!(
            output.unwrap_err().to_string(),
            r#"command ["sh", "-c", "sleep 0.02 && echo \'Goodbye, World!\'"] exited with code <signal 9>"#
        );
    }

    #[test]
    fn post_wait() {
        let post_wait_called = Arc::new(Mutex::new(false));
        let post_wait = {
            let post_wait_called = Arc::clone(&post_wait_called);
            move || {
                *post_wait_called.lock().unwrap() = true;
                Ok(())
            }
        };

        let mut handle = sleeping_handle();
        handle.set_post_wait(post_wait);

        assert!(!*post_wait_called.lock().unwrap());
        handle.wait().unwrap();
        assert!(*post_wait_called.lock().unwrap());
    }

    #[test]
    fn post_kill() {
        let post_kill_called = Arc::new(Mutex::new(false));
        let post_kill = {
            let post_kill_called = Arc::clone(&post_kill_called);
            move || {
                *post_kill_called.lock().unwrap() = true;
                Ok(())
            }
        };

        let mut handle = sleeping_handle();
        handle.set_post_kill(post_kill);

        assert!(!*post_kill_called.lock().unwrap());
        handle.kill().unwrap();
        assert!(*post_kill_called.lock().unwrap());
    }
}
