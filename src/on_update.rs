// Standard Library Imports
use std::{
    fmt::{self, Debug, Formatter},
    sync::{Arc, RwLock},
};

// Public API ==========================================================================================================

#[derive(Clone, Default)]
pub struct OnUpdate(Arc<RwLock<Option<OnUpdateCallback>>>);

impl OnUpdate {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn set(&self, on_update: OnUpdateCallback) {
        *self.0.write().unwrap() = Some(on_update);
    }

    pub fn clear(&self) {
        *self.0.write().unwrap() = None;
    }

    pub fn call(&self) {
        if let Some(ref on_update) = *self.0.read().unwrap() {
            on_update();
        }
    }
}

pub type OnUpdateCallback = Arc<dyn Fn() + Send + Sync>;

impl Debug for OnUpdate {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        #[derive(Debug)]
        struct OnUpdateCallback;

        let option = self.0.read().unwrap().as_ref().map(|_| OnUpdateCallback);
        write!(f, "{:?}", Arc::new(RwLock::new(option)))
    }
}

// Unit Tests ==========================================================================================================

#[cfg(test)]
pub mod tests {
    use std::sync::Mutex;

    use super::*;

    #[test]
    fn call() {
        let updates = Arc::new(Mutex::new(Vec::new()));
        let on_update = OnUpdate::new();

        // No callback set (`Default` impl)
        on_update.call();
        assert_eq!(updates.lock().unwrap()[..], Vec::<&str>::new());

        // Set a callback and call it a couple of times
        on_update.set(Arc::new({
            let updates = Arc::clone(&updates);
            move || updates.lock().unwrap().push("first_callback")
        }));
        on_update.call();
        on_update.call();
        assert_eq!(
            updates.lock().unwrap()[..],
            ["first_callback", "first_callback"]
        );

        // Clear the callback and call it a couple of times
        on_update.clear();
        on_update.call();
        on_update.call();
        assert_eq!(
            updates.lock().unwrap()[..],
            ["first_callback", "first_callback"]
        );

        // Set a new callback and call it once
        on_update.set(Arc::new({
            let updates = Arc::clone(&updates);
            move || updates.lock().unwrap().push("second_callback")
        }));
        on_update.call();
        assert_eq!(
            updates.lock().unwrap()[..],
            ["first_callback", "first_callback", "second_callback"]
        );
    }
}
