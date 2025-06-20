use dioxus::prelude::*;

use crate::components::optionally_valid_input::OptionallyValidInput;

#[component]
pub fn LaunchInterval(value: Option<u64>, oninput: EventHandler<Option<u64>>) -> Element {
    rsx! {
        label { class: "col-span-3 grid grid-cols-subgrid input w-full",
            span { class: "label tooltip",
                "Launch Interval"

                p { class: "tooltip-content",
                    "If several jobs are started at the same time, then there is a chance that some of them will \
                     spontaneously fail. These failures are caused by different Byos instances clashing during startup \
                     and can be resolved by staggering the launch of new jobs by some (empirically determined) number \
                     of seconds"
                }
            }
            OptionallyValidInput {
                value,
                oninput,
                min: "0",
                r#type: "number",
                required: "true",
            }
            span { class: "label",
                if value == Some(1) {
                    "second"
                } else {
                    "seconds"
                }
            }
        }
    }
}
