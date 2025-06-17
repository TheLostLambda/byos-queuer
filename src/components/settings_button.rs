// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use crate::components::{settings_icon::SettingsIcon, settings_modal::SettingsModal};
use dioxus::prelude::*;

use byos_queuer::queue::Status as QueueStatus;

#[component]
pub fn SettingsButton(status: QueueStatus) -> Element {
    const MODAL_ID: &str = "settings_modal";

    rsx! {
        button {
            class: "btn btn-square",
            onclick: |_| {
                document::eval(&format!("{MODAL_ID}.showModal()"));
            },
            disabled: status.running(),
            SettingsIcon {}
        }
        SettingsModal { id: MODAL_ID }
    }
}
