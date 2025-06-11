use byos_queuer::job::Status;
use dioxus::prelude::*;

use super::status_badge::StatusProp;
use crate::components::{delete::Delete, reset::Reset, status_badge::StatusBadge};

#[component]
pub fn Job(index: usize, name: String, #[props(into)] status: StatusProp) -> Element {
    let show_reset = !matches!(&status.0, Status::Queued);

    rsx! {
        li { class: "list-row items-center",
            div { class: "font-mono list-col-grow", {name} }

            StatusBadge { status }

            if show_reset {
                Reset { index }
            }

            Delete { index }
        }
    }
}
