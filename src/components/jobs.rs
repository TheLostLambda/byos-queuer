use dioxus::prelude::*;

use crate::components::job::Job;

#[component]
pub fn Jobs() -> Element {
    rsx! {
        h2 {
            class: "card-title",
            "Queued Jobs"
        }
        ol {
            class: "list bg-base-100 rounded-box shadow-md",
            Job { name: "First Job" }
            Job { name: "Second Job" }
            Job { name: "Third Job" }
        }
    }
}
