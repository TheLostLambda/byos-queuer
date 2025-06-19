use dioxus::prelude::*;

use crate::{
    QUEUE,
    components::{RunBar, job::Job, new_job::NewJob, top_bar::TopBar},
};

#[component]
pub fn JobQueue() -> Element {
    use_hook(|| {
        QUEUE.read().unwrap().set_on_update(schedule_update());
    });

    let queue = QUEUE.read().unwrap();
    let jobs = queue.jobs();
    let status = queue.status();
    drop(queue);

    rsx! {
        div { class: "flex flex-col card-body",
            TopBar { status }
            ol { class: "list bg-base-100 rounded-box",

                for (index , (name , status)) in jobs.into_iter().enumerate() {
                    Job { index, name, status }
                }

                NewJob {}
            }
            RunBar { status }
        }
    }
}
