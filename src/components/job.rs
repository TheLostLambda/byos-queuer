use dioxus::prelude::*;

use crate::components::{delete::Delete, reset::Reset};

#[component]
pub fn Job(name: String) -> Element {
    rsx! {
        li {
            class: "list-row",
            div {
                class: "font-mono list-col-grow",
                { name }
            }
            Reset {}
            Delete {}
        }
    }
}
