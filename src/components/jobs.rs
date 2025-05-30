use dioxus::prelude::*;

use crate::{STATE, components::job::Job};

#[component]
pub fn Jobs() -> Element {
    use_hook(|| {
        STATE
            .lock()
            .unwrap()
            .set_on_update(schedule_update())
            .unwrap();
    });

    rsx! {
        h2 {
            class: "card-title",
            "Queued Jobs"
        }
        ol {
            class: "list bg-base-100 rounded-box shadow-md",

            for (name, _) in STATE.lock().unwrap().jobs() {
                Job { name }
            }
        }
    }
}
