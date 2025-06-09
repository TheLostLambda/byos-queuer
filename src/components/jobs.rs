use dioxus::prelude::*;

use crate::{
    STATE,
    components::{RunBar, job::Job},
};

#[component]
pub fn Jobs() -> Element {
    use_hook(|| {
        STATE.read().unwrap().set_on_update(schedule_update());
    });

    rsx! {
        div {
            class: "flex flex-col card-body",
            h2 {
                class: "card-title",
                "Queued Jobs"
            }
            ol {
                class: "list bg-base-100 rounded-box",

                for (index, (name, status)) in STATE.read().unwrap().jobs().into_iter().enumerate() {
                    Job { index, name, status }
                }
            }
            RunBar { running: STATE.read().unwrap().running(), ready: STATE.read().unwrap().ready() }
        }
    }
}
