// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use byos_queuer::queue::Status as QueueStatus;
use dioxus::prelude::*;

use crate::STATE;

enum OnClick {
    Run,
    Cancel,
    Nothing,
}

#[component]
pub fn RunButton(status: QueueStatus) -> Element {
    use OnClick::*;
    use QueueStatus::*;

    let (color_class, content, onclick) = match status {
        Running => ("btn-error", rsx! { "Cancel" }, Cancel),
        Starting => ("btn-warning", rsx! { "Starting..." }, Cancel),
        Stopping => ("btn-warning", rsx! { "Stopping..." }, Nothing),
        Ready | Paused => ("btn-success", rsx! { "Run" }, Run),
        Empty | Finished => ("btn-success", rsx! { "Run" }, Nothing),
    };

    rsx! {
        button {
            class: "btn btn-block {color_class} text-lg",
            disabled: matches!(status, Finished | Empty),
            onclick: move |_| {
                let queue = STATE.read().unwrap();
                match onclick {
                    Run => queue.run().unwrap(),
                    Cancel => queue.cancel().unwrap(),
                    Nothing => {}
                }
            },

            {content}
        }
    }
}
