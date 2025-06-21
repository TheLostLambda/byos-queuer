use std::{process::Output, sync::Mutex};

use color_eyre::Result;

pub struct Handle {
    inner: duct::Handle,
    killed: Mutex<bool>,
    post_kill: Option<Hook>,
    post_finish: Option<Hook>,
}

pub type Hook = Box<dyn Fn(Result<Output>) -> Result<Output> + Send + Sync>;

impl Handle {
    pub fn post_kill(&mut self, post_kill: Hook) -> &mut Self {
        self.post_kill = Some(post_kill);
        self
    }

    pub fn post_finish(&mut self, post_finish: Hook) -> &mut Self {
        self.post_finish = Some(post_finish);
        self
    }

    pub fn kill(&self) -> Result<()> {
        // NOTE: The `post_kill` hook isn't called here, since this just sends a kill signal to the child but doesn't
        // actually wait for it to die — instead, we set a flag so that `.wait()` knows to call `post_kill` instead of
        // the normal `post_finish` hook
        *self.killed.lock().unwrap() = true;
        self.inner.kill().map_err(Into::into)
    }

    pub fn wait(&self) -> Result<Output> {
        let result = self.inner.wait().map(ToOwned::to_owned).map_err(Into::into);

        if *self.killed.lock().unwrap() {
            if let Some(post_kill) = &self.post_kill {
                return post_kill(result);
            }
        } else if let Some(post_finish) = &self.post_finish {
            return post_finish(result);
        }

        result
    }
}

impl From<duct::Handle> for Handle {
    fn from(value: duct::Handle) -> Self {
        Self {
            inner: value,
            killed: Mutex::new(false),
            post_kill: None,
            post_finish: None,
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
        assert!(handle.wait().is_err());

        assert!(*post_kill_called.lock().unwrap());
    }

    #[test]
    fn post_finish() {
        let post_finish_called = Arc::new(Mutex::new(false));
        let post_finish = {
            let post_finish_called = Arc::clone(&post_finish_called);
            Box::new(move |result| {
                *post_finish_called.lock().unwrap() = true;
                result
            })
        };

        let mut handle = sleeping_handle();
        handle.post_finish(post_finish);

        assert!(!*post_finish_called.lock().unwrap());
        handle.wait().unwrap();
        assert!(*post_finish_called.lock().unwrap());
    }
}
