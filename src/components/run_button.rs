// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use byos_queuer::queue::Status as QueueStatus;
use dioxus::prelude::*;

use crate::STATE;

#[component]
pub fn RunButton(status: QueueStatus) -> Element {
    let (color_class, content) = match status {
        QueueStatus::Running => ("btn-error", rsx! { "Cancel" }),
        QueueStatus::Stopping => ("btn-warning", rsx! { "Stopping..." }),
        _ => ("btn-success", rsx! { "Run" }),
    };

    rsx! {
        button {
            class: "btn btn-block {color_class} text-lg",
            disabled: status == QueueStatus::Finished,
            onclick: move |_| {
                let queue = STATE.read().unwrap();
                if status == QueueStatus::Running {
                    queue.cancel().unwrap();
                } else {
                    queue.run().unwrap();
                }
            },

            {content}
        }
    }
}
