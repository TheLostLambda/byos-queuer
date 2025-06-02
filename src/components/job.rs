use dioxus::prelude::*;

use crate::components::delete::Delete;

#[component]
pub fn Job(name: String) -> Element {
    rsx! {
        li {
            class: "list-row",
            div {
                class: "font-mono list-col-grow",
                { name }
            }
            Delete {}
        }
    }
}
