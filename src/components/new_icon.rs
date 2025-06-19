// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;

#[component]
pub fn NewIcon(#[props(default = 24)] size: usize) -> Element {
    rsx! {
        svg {
            class: "lucide lucide-circle-plus-icon lucide-circle-plus",
            fill: "none",
            height: size,
            stroke: "currentColor",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            stroke_width: "2",
            view_box: "0 0 24 24",
            width: size,
            xmlns: "http://www.w3.org/2000/svg",
            circle { cx: "12", cy: "12", r: "10" }
            path { d: "M8 12h8" }
            path { d: "M12 8v8" }
        }
    }
}
