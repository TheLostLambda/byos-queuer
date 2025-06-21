use byos_queuer::JobStatus;
use dioxus::prelude::*;

use crate::components::{
    delete_job::DeleteJob,
    job_status_badge::{JobStatusBadge, JobStatusProp},
    reset_job::ResetJob,
};

#[component]
pub fn Job(index: usize, name: String, #[props(into)] status: JobStatusProp) -> Element {
    use JobStatus::*;

    let show_reset = matches!(&status.0, Running(..) | Completed(..) | Failed(..));

    rsx! {
        li { class: "list-row items-center",
            div { class: "font-mono list-col-grow", {name} }

            JobStatusBadge { status }

            if show_reset {
                ResetJob { index }
            }

            DeleteJob { index }
        }
    }
}
