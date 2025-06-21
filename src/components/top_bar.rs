// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use byos_queuer::QueueStatus;
use dioxus::prelude::*;

use crate::components::{clear_queue::ClearQueue, reset_queue::ResetQueue};

#[component]
pub fn TopBar(status: QueueStatus) -> Element {
    rsx! {
        div { class: "flex items-center justify-between gap-4 px-4",
            h2 { class: "card-title grow", "Queued Jobs" }

            if status.resettable() {
                ResetQueue {}
            }

            if status.clearable() {
                ClearQueue {}
            }
        }
    }
}
