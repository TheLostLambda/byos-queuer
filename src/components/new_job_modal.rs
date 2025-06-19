// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;

use crate::components::group_samples::GroupSamples;

#[component]
pub fn NewJobModal(id: &'static str) -> Element {
    let _base_workflow = use_signal(|| ());
    let _sample_files = use_signal(|| ());
    let _protein_file = use_signal(|| ());
    let _modifications_file = use_signal(|| ());
    let _output_directory = use_signal(|| ());
    let grouped = use_signal(|| false);

    let close_modal = move || {
        document::eval(&format!("{id}.close()"));
    };

    let onsubmit = move |_| {
        close_modal();
    };

    rsx! {
        dialog { class: "modal", id,
            form {
                class: "modal-box overflow-visible flex flex-col items-stretch gap-4",
                method: "dialog",
                onsubmit,

                h3 { class: "text-lg font-bold text-center mb-1", "Queue Job(s)" }

                GroupSamples { value: grouped }

                div { class: "modal-action mt-2",
                    button {
                        class: "btn grow",
                        r#type: "button",
                        formnovalidate: true,
                        onclick: move |_| close_modal(),
                        "Cancel"
                    }
                    button { class: "btn grow btn-primary", r#type: "submit", "Queue" }
                }
            }
        }
    }
}
