// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;

use crate::components::{
    launch_interval::LaunchInterval, maximum_concurrent_jobs::MaximumConcurrentJobs,
};

#[component]
pub fn SettingsModal(id: &'static str) -> Element {
    rsx! {
        dialog { class: "modal", id,
            form {
                method: "dialog",
                class: "modal-box overflow-visible flex flex-col items-stretch gap-4",
                h3 { class: "text-lg font-bold text-center mb-1", "Queue Settings" }

                MaximumConcurrentJobs {}
                LaunchInterval {}

                div { class: "modal-action mt-2",
                    button { class: "btn btn-block", "Close" }
                }
            }
        }
    }
}
