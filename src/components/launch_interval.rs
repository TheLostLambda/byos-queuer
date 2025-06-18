use dioxus::prelude::*;

#[component]
pub fn LaunchInterval() -> Element {
    rsx! {
        div {
            label { class: "input validator w-full",
                span { class: "label tooltip",
                    "Launch Interval"

                    p { class: "tooltip-content",
                        "If several jobs are started at the same time, then there is a chance that some of them will \
                        spontaneously fail. These failures are caused by different Byos instances clashing during \
                        startup and can be resolved by staggering the launch of new jobs by some (empirically \
                        determined) number of seconds."
                    }
                }
                input { min: "0", r#type: "number", required: "true" }
                span { class: "label", "seconds" }
            }
            p { class: "hidden validator-hint", "Must be zero or some positive integer" }
        }
    }
}
