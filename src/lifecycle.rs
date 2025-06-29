use std::{
    fmt::Debug,
    io::{ErrorKind, Result},
    process::{Command, ExitStatus},
};

use process_wrap::std::{StdChildWrapper, StdCommandWrap, StdCommandWrapper};

#[derive(Debug, Default)]
pub struct Lifecycle {
    pre_spawn: Hook,
    post_kill: Hook,
    post_finish: Hook,
}

#[derive(Default)]
pub struct Hook(Option<DynHook>);
pub type DynHook = Box<dyn Fn() -> color_eyre::Result<()> + Send + Sync>;

impl Hook {
    fn call(&self) {
        if let Some(hook) = &self.0 {
            hook();
        }
    }
}

impl From<DynHook> for Hook {
    fn from(value: DynHook) -> Self {
        Self(Some(value))
    }
}

impl Debug for Hook {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "{}", if self.0.is_some() { "<set>" } else { "<unset>" })
    }
}

macro_rules! hook_setter {
    ($method:ident) => {
        pub fn $method(self, $method: impl Into<Hook>) -> Self {
            Self {
                $method: $method.into(),
                ..self
            }
        }
    };
}

impl Lifecycle {
    pub fn new() -> Self {
        Self::default()
    }

    hook_setter!(pre_spawn);
    hook_setter!(post_kill);
    hook_setter!(post_finish);
}

impl StdCommandWrapper for Lifecycle {
    fn pre_spawn(&mut self, _command: &mut Command, _core: &StdCommandWrap) -> Result<()> {
        self.pre_spawn.call();
        Ok(())
    }

    fn wrap_child(
        &mut self,
        child: Box<dyn StdChildWrapper>,
        _core: &StdCommandWrap,
    ) -> Result<Box<dyn StdChildWrapper>> {
        Ok(child)
    }
}

#[derive(Debug)]
pub struct LifecycleChild {
    inner: Box<dyn StdChildWrapper>,
    killed: bool,
    post_kill: Hook,
    post_finish: Hook,
}

impl StdChildWrapper for LifecycleChild {
    fn inner(&self) -> &dyn StdChildWrapper {
        &*self.inner
    }

    fn inner_mut(&mut self) -> &mut dyn StdChildWrapper {
        &mut *self.inner
    }

    fn into_inner(self: Box<Self>) -> Box<dyn StdChildWrapper> {
        self.inner
    }

    fn start_kill(&mut self) -> Result<()> {
        self.killed = true;
        self.inner_mut().start_kill()
    }

    fn wait(&mut self) -> Result<ExitStatus> {
        let result = self.inner_mut().wait();

        if self.killed {
            self.post_kill.call();
        } else {
            self.post_finish.call();
        }

        if let Ok(exit_status) = result
            && !exit_status.success()
        {
            return Err(ErrorKind::Other.into());
        } else {
            result
        }
    }
}

// #[cfg(test)]
// mod tests {
//     use std::{
//         sync::{Arc, Mutex},
//         time::{Duration, Instant},
//     };

//     use process_wrap::std::StdCommandWrap;

//     use super::*;

//     fn sleeping_child(
//         post_kill: Option<Hook>,
//         post_wait: Option<Hook>,
//     ) -> Box<dyn StdChildWrapper> {
//         // TODO: Use conditional compilation to adapt this command for Windows
//         StdCommandWrap::with_new("sh", |c| {
//             c.args(["-c", "sleep 0.02 && echo 'Goodbye, World!'"]);
//         })
//         .wrap(wrapper)
//         .spawn()
//         .unwrap()
//     }

//     #[test]
//     fn wait() {
//         let start = Instant::now();
//         let handle = sleeping_child();
//         let output = handle.wait().unwrap();
//         let elapsed = start.elapsed();

//         assert_eq!(output.status.code(), Some(0));
//         assert_eq!(output.stdout, b"Goodbye, World!\n");
//         assert_eq!(output.stderr, b"");
//         assert!(elapsed > Duration::from_millis(20));
//         assert!(elapsed < Duration::from_millis(25));
//     }

//     #[test]
//     fn kill() {
//         let handle = sleeping_child();
//         handle.kill().unwrap();
//         let output = handle.wait();

//         assert_eq!(
//             output.unwrap_err().to_string(),
//             r#"command ["sh", "-c", "sleep 0.02 && echo \'Goodbye, World!\'"] exited with code <signal 9>"#
//         );
//     }

//     #[test]
//     fn post_kill() {
//         let post_kill_called = Arc::new(Mutex::new(false));
//         let post_kill = {
//             let post_kill_called = Arc::clone(&post_kill_called);
//             Box::new(move |result| {
//                 *post_kill_called.lock().unwrap() = true;
//                 result
//             })
//         };

//         let mut handle = sleeping_child();
//         handle.post_kill(post_kill);

//         assert!(!*post_kill_called.lock().unwrap());

//         handle.kill().unwrap();
//         assert!(handle.wait().is_err());

//         assert!(*post_kill_called.lock().unwrap());
//     }

//     #[test]
//     fn post_finish() {
//         let post_finish_called = Arc::new(Mutex::new(false));
//         let post_finish = {
//             let post_finish_called = Arc::clone(&post_finish_called);
//             Box::new(move |result| {
//                 *post_finish_called.lock().unwrap() = true;
//                 result
//             })
//         };

//         let mut handle = sleeping_child();
//         handle.post_finish(post_finish);

//         assert!(!*post_finish_called.lock().unwrap());
//         handle.wait().unwrap();
//         assert!(*post_finish_called.lock().unwrap());
//     }
// }
