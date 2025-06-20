use std::process::Output;

use color_eyre::Result;

pub struct Handle {
    inner: duct::Handle,
    post_kill: Option<Hook<()>>,
    post_wait: Option<Hook<Output>>,
}

pub type Hook<T> = Box<dyn Fn(Result<T>) -> Result<T> + Send + Sync>;

impl Handle {
    pub fn post_kill(&mut self, post_kill: Hook<()>) -> &mut Self {
        self.post_kill = Some(post_kill);
        self
    }

    pub fn post_wait(&mut self, post_wait: Hook<Output>) -> &mut Self {
        self.post_wait = Some(post_wait);
        self
    }

    pub fn kill(&self) -> Result<()> {
        let result = self.inner.kill().map_err(Into::into);

        if let Some(post_kill) = &self.post_kill {
            post_kill(result)
        } else {
            result
        }
    }

    pub fn wait(&self) -> Result<Output> {
        let result = self.inner.wait().map(ToOwned::to_owned).map_err(Into::into);

        if let Some(post_wait) = &self.post_wait {
            post_wait(result)
        } else {
            result
        }
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
    fn post_kill() {
        let post_kill_called = Arc::new(Mutex::new(false));
        let post_kill = {
            let post_kill_called = Arc::clone(&post_kill_called);
            Box::new(move |result| {
                *post_kill_called.lock().unwrap() = true;
                result
            })
        };

        let mut handle = sleeping_handle();
        handle.post_kill(post_kill);

        assert!(!*post_kill_called.lock().unwrap());
        handle.kill().unwrap();
        assert!(*post_kill_called.lock().unwrap());
    }

    #[test]
    fn post_wait() {
        let post_wait_called = Arc::new(Mutex::new(false));
        let post_wait = {
            let post_wait_called = Arc::clone(&post_wait_called);
            Box::new(move |result| {
                *post_wait_called.lock().unwrap() = true;
                result
            })
        };

        let mut handle = sleeping_handle();
        handle.post_wait(post_wait);

        assert!(!*post_wait_called.lock().unwrap());
        handle.wait().unwrap();
        assert!(*post_wait_called.lock().unwrap());
    }
}
