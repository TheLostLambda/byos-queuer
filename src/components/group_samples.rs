// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;

#[component]
pub fn GroupSamples(value: Signal<bool>) -> Element {
    rsx! {
        label { class: "flex justify-between w-full",
            span { class: "label tooltip",
                "Group Samples"

                p { class: "tooltip-content",
                    "By default, every sample file is split out into its own job. This speeds up computation by \
                     bypassing Byos's slow cross-sample correlation step, but where you're processing replicates and \
                     want their search results combined, it can be toggled back on"
                }
            }
            input {
                class: "toggle",
                value,
                oninput: move |event| value.set(event.value().parse().unwrap()),
                r#type: "checkbox",
            }
        }
    }
}
