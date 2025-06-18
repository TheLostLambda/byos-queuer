// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use std::time::Duration;

use dioxus::prelude::*;

use crate::{
    QUEUE,
    components::{launch_interval::LaunchInterval, maximum_concurrent_jobs::MaximumConcurrentJobs},
};

#[component]
pub fn SettingsModal(id: &'static str) -> Element {
    let read_workers = || QUEUE.read().unwrap().workers().to_string();
    let read_stagger_duration = || {
        QUEUE
            .read()
            .unwrap()
            .stagger_duration()
            .as_secs()
            .to_string()
    };
    let mut workers = use_signal(read_workers);
    let mut stagger_duration = use_signal(read_stagger_duration);

    let mut close_modal = move || {
        document::eval(&format!("{id}.close()"));

        // NOTE: When we close the dialog, re-sync the input states with the actual values from `QUEUE`. This ensures
        // that reopening the modal will always show the values currently being used by `QUEUE, even if the user just
        // inputted nonsense and then closed the modal without submitting.
        workers.set(read_workers());
        stagger_duration.set(read_stagger_duration());
    };

    let onsubmit = move |_| {
        let workers = workers().parse().unwrap();
        let stagger_duration = Duration::from_secs(stagger_duration().parse().unwrap());

        let mut queue = QUEUE.write().unwrap();
        queue.set_workers(workers).unwrap();
        queue.set_stagger_duration(stagger_duration).unwrap();
        drop(queue);

        close_modal();
    };

    rsx! {
        dialog { class: "modal", id,
            form {
                class: "modal-box overflow-visible flex flex-col items-stretch gap-4",
                method: "dialog",
                onsubmit,

                h3 { class: "text-lg font-bold text-center mb-1", "Queue Settings" }

                MaximumConcurrentJobs { value: workers }
                LaunchInterval { value: stagger_duration }

                div { class: "modal-action mt-2",
                    button {
                        class: "btn grow",
                        r#type: "button",
                        formnovalidate: true,
                        onclick: move |_| close_modal(),
                        "Cancel"
                    }
                    button { class: "btn grow btn-primary", r#type: "submit", "Save" }
                }
            }
        }
    }
}
