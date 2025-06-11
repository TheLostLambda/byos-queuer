use byos_queuer::job::Status as JobStatus;
use dioxus::prelude::*;

use crate::components::{
    delete::Delete,
    job_status_badge::{JobStatusBadge, JobStatusProp},
    reset::Reset,
};

#[component]
pub fn Job(index: usize, name: String, #[props(into)] status: JobStatusProp) -> Element {
    let show_reset = !matches!(&status.0, JobStatus::Queued);

    rsx! {
        li { class: "list-row items-center",
            div { class: "font-mono list-col-grow", {name} }

            JobStatusBadge { status }

            if show_reset {
                Reset { index }
            }

            Delete { index }
        }
    }
}
