use dioxus::prelude::*;

use super::status_badge::StatusProp;
use crate::components::{delete::Delete, status_badge::StatusBadge};

#[component]
pub fn Job(index: usize, name: String, #[props(into)] status: StatusProp) -> Element {
    rsx! {
        li { class: "list-row items-center",
            div { class: "font-mono list-col-grow", {name} }

            StatusBadge { status }
            Delete { index }
        }
    }
}
