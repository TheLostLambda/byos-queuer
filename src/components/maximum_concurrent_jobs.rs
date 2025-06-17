use dioxus::prelude::*;

#[component]
pub fn MaximumConcurrentJobs() -> Element {
    rsx! {
        div {
            label { class: "input validator w-full",
                span { class: "label", "Maximum Concurrent Jobs" }
                input { min: "1", r#type: "number", required: "true" }
            }
            p { class: "hidden validator-hint", "Must be a positive integer" }
        }
    }
}
