// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;

#[component]
pub fn LaunchInterval(value: Signal<String>) -> Element {
    let singular = use_memo(move || value().parse() == Ok(1));

    rsx! {
        label { class: "col-span-3 grid grid-cols-subgrid input w-full",
            span { class: "label tooltip",
                "Launch Interval"

                p { class: "tooltip-content",
                    "If several jobs are started at the same time, then there is a chance that some of them will \
                        spontaneously fail. These failures are caused by different Byos instances clashing during \
                        startup and can be resolved by staggering the launch of new jobs by some (empirically \
                        determined) number of seconds."
                }
            }
            input {
                value,
                oninput: move |event| value.set(event.value()),
                min: "0",
                r#type: "number",
                required: "true",
            }
            span { class: "label",
                if singular() {
                    "second"
                } else {
                    "seconds"
                }
            }
        }
    }
}
