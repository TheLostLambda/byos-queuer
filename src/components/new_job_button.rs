// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use byos_queuer::queue::Status as QueueStatus;
use dioxus::prelude::*;

use crate::components::{new_icon::NewIcon, new_job_modal::NewJobModal};

#[component]
pub fn NewJobButton(status: QueueStatus) -> Element {
    const MODAL_ID: &str = "new_job_modal";

    let (height_class, content) = if status.empty() {
        (
            "h-32",
            rsx! {
                NewIcon { size: 48 }
                span { class: "text-3xl", "Queue a job" }
            },
        )
    } else {
        (
            "",
            rsx! {
                NewIcon {}
            },
        )
    };

    rsx! {
        li { class: "list-row",
            button {
                class: "list-col-grow btn btn-block gap-4 {height_class}",
                onclick: |_| {
                    document::eval(&format!("{MODAL_ID}.showModal()"));
                },
                {content}
            }
            NewJobModal { id: MODAL_ID }
        }
    }
}
