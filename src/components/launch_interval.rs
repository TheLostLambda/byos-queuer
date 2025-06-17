use dioxus::prelude::*;

#[component]
pub fn LaunchInterval() -> Element {
    rsx! {
        div {
            label { class: "input validator w-full",
                span { class: "label", "Launch Interval" }
                input { min: "0", r#type: "number", required: "true" }
                span { class: "label", "seconds" }
            }
            p { class: "hidden validator-hint", "Must be zero or some positive integer" }
        }
    }
}
