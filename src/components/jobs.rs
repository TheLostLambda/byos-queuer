use dioxus::prelude::*;

use crate::{STATE, components::job::Job};

#[component]
pub fn Jobs() -> Element {
    use_hook(|| {
        STATE.read().unwrap().set_on_update(schedule_update());
    });

    rsx! {
        h2 {
            class: "card-title",
            "Queued Jobs"
        }
        ol {
            class: "list bg-base-100 rounded-box shadow-md",

            for (index, (name, _)) in STATE.read().unwrap().jobs().into_iter().enumerate() {
                Job { index, name }
            }
        }
    }
}
