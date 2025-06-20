use dioxus::prelude::*;

use crate::components::optionally_valid_input::OptionallyValidInput;

#[component]
pub fn MaximumConcurrentJobs(
    value: Option<usize>,
    oninput: EventHandler<Option<usize>>,
) -> Element {
    rsx! {
        label { class: "col-span-3 grid grid-cols-subgrid input w-full",
            span { class: "label tooltip",
                "Maximum Concurrent Jobs"

                p { class: "tooltip-content",
                    "Limits the number of jobs that can be run at once. Increasing this number can lead to higher \
                     throughput (more jobs completed per unit of time), but individual jobs will take longer to \
                     complete, and — if this parameter is set too high — the computer could run out of memory"
                }
            }
            OptionallyValidInput {
                class: "col-span-2",
                value,
                oninput,
                min: "1",
                r#type: "number",
                required: "true",
            }
        }
    }
}
