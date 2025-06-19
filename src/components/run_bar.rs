// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use byos_queuer::queue::Status as QueueStatus;
use dioxus::prelude::*;

use crate::components::{run_button::RunButton, settings_button::SettingsButton};

#[component]
pub fn RunBar(status: QueueStatus) -> Element {
    rsx! {
        div { class: "flex items-center justify-between gap-4 mx-4",

            RunButton { status }
            SettingsButton { status }
        }
    }
}
