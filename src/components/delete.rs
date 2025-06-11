// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;

use crate::STATE;

#[component]
pub fn Delete(index: usize) -> Element {
    rsx! {
        div {
            class: "tooltip",
            "data-tip": "Delete this job",
            onclick: move |_| STATE.read().unwrap().remove_job(index).unwrap(),

            svg {
                xmlns: "http://www.w3.org/2000/svg",
                width: "24",
                height: "24",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                class: "lucide lucide-circle-x-icon lucide-circle-x",

                circle {
                    cx: "12",
                    cy: "12",
                    r: "10"
                }

                path {
                    d: "m15 9-6 6",
                }

                path {
                    d: "m9 9 6 6",
                }
            }
        }
    }
}
