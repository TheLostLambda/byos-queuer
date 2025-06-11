// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;

use crate::STATE;

#[component]
pub fn Reset(index: usize) -> Element {
    rsx! {
        div {
            class: "tooltip",
            "data-tip": "Reset this job",
            onclick: move |_| STATE.read().unwrap().reset_job(index).unwrap(),

            svg {
                class: "lucide lucide-rotate-ccw-icon lucide-rotate-ccw",
                fill: "none",
                height: "24",
                stroke: "currentColor",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                stroke_width: "2",
                view_box: "0 0 24 24",
                width: "24",
                xmlns: "http://www.w3.org/2000/svg",
                path { d: "M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" }
                path { d: "M3 3v5h5" }
            }
        }
    }
}
