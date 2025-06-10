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

    let queue = STATE.read().unwrap();
    let jobs = queue.jobs();
    let running = queue.running();
    let ready = queue.ready();
    drop(queue);

    rsx! {
        div {
            class: "flex flex-col card-body",
            h2 {
                class: "card-title",
                "Queued Jobs"
            }
            ol {
                class: "list bg-base-100 rounded-box",

                for (index, (name, status)) in jobs.into_iter().enumerate() {
                    Job { index, name, status }
                }
            }
            RunBar { running, ready }
        }
    }
}
