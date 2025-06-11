// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;

use crate::STATE;

#[component]
pub fn RunButton(running: bool, finished: bool) -> Element {
    let color_class = if running { "btn-error" } else { "btn-success" };

    rsx! {
        button {
            class: "btn btn-block {color_class}",
            disabled: finished,
            onclick: move |_| {
                let queue = STATE.read().unwrap();
                if running {
                    queue.cancel().unwrap();
                } else {
                    queue.run().unwrap();
                }
            },

            if running {
                "Cancel"
            } else {
                "Run"
            }
        }
    }
}
