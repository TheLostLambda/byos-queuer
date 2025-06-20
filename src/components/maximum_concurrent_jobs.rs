// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;

#[component]
pub fn MaximumConcurrentJobs(value: Signal<String>) -> Element {
    rsx! {
        label { class: "col-span-3 grid grid-cols-subgrid input w-full",
            span { class: "label tooltip",
                "Maximum Concurrent Jobs"

                p { class: "tooltip-content",
                    "Limits the number of jobs that can be run at once. Increasing this number can lead to higher \
                        throughput (more jobs completed per unit of time), but individual jobs will take longer to \
                        complete, and — if this parameter is set too high — the computer could run out of memory."
                }
            }
            input {
                class: "col-span-2",
                value,
                oninput: move |event| value.set(event.value()),
                min: "1",
                r#type: "number",
                required: "true",
            }
        }
    }
}
