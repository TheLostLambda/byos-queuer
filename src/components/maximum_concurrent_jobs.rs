use dioxus::prelude::*;

#[component]
pub fn MaximumConcurrentJobs() -> Element {
    rsx! {
        div {
            label { class: "input validator w-full",
                span { class: "label tooltip",
                    "Maximum Concurrent Jobs"

                    p { class: "tooltip-content",
                        "Limits the number of jobs that can be run at once. Increasing this number can lead to higher \
                        throughput (more jobs completed per unit of time), but individual jobs will take longer to \
                        complete, and — if this parameter is set too high — the computer could run out of memory."
                    }
                }
                input { min: "1", r#type: "number", required: "true" }
            }
            p { class: "hidden validator-hint", "Must be a positive integer" }
        }
    }
}
