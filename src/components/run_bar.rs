// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;

use crate::components::run_button::RunButton;

#[component]
pub fn RunBar(running: bool, ready: bool) -> Element {
    rsx! {
        div {
            class: "flex items-center justify-between",

            RunButton { running, ready }
        }
    }
}
