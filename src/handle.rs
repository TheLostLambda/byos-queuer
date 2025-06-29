use std::sync::{Arc, Mutex};

use color_eyre::Result;
use process_wrap::std::StdChildWrapper;

#[derive(Clone)]
pub struct Handle(Arc<Mutex<Box<dyn StdChildWrapper>>>);

impl From<Box<dyn StdChildWrapper>> for Handle {
    fn from(value: Box<dyn StdChildWrapper>) -> Self {
        Self(Arc::new(Mutex::new(value)))
    }
}

impl Handle {
    pub fn kill(&self) -> Result<()> {
        self.0.lock().unwrap().start_kill().map_err(Into::into)
    }

    pub fn wait(&self) -> Result<()> {
        self.0
            .lock()
            .unwrap()
            .wait()
            .map(|_| ())
            .map_err(Into::into)
    }
}
