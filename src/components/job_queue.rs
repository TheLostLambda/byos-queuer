use dioxus::prelude::*;

use crate::{
    STATE,
    components::{RunBar, job::Job, top_bar::TopBar},
};

#[component]
pub fn JobQueue() -> Element {
    use_hook(|| {
        STATE.read().unwrap().set_on_update(schedule_update());
    });

    let queue = STATE.read().unwrap();
    let jobs = queue.jobs();
    let status = queue.status();
    drop(queue);

    rsx! {
        div { class: "flex flex-col card-body",
            TopBar {}
            ol { class: "list bg-base-100 rounded-box",

                for (index , (name , status)) in jobs.into_iter().enumerate() {
                    Job { index, name, status }
                }
            }
            RunBar { status }
        }
    }
}
