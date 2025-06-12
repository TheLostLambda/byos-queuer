// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;

use crate::{STATE, components::reset_icon::ResetIcon};

#[component]
pub fn ResetJob(index: usize) -> Element {
    rsx! {
        div {
            class: "tooltip",
            "data-tip": "Reset job",
            onclick: move |_| STATE.read().unwrap().reset_job(index).unwrap(),

            ResetIcon {}
        }
    }
}
