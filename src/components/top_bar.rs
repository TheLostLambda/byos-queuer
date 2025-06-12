// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use byos_queuer::queue::Status as QueueStatus;
use dioxus::prelude::*;

use crate::components::{clear_queue::ClearQueue, reset_queue::ResetQueue};

#[component]
pub fn TopBar(status: QueueStatus) -> Element {
    use QueueStatus::*;

    // TODO: If I write a lot of code like this, I should move it to a `QueueStatus` method. Something like
    // `QueueStatus.resettable()` or `QueueStatus.clearable()`.
    let resettable = matches!(status, Running | Stopping | Paused | Restarting | Finished);

    rsx! {
        div { class: "flex items-center justify-between gap-4 px-4",
            h2 { class: "card-title grow", "Queued Jobs" }

            if resettable {
                ResetQueue {}
            }

            if status != Empty {
                ClearQueue {}
            }
        }
    }
}
