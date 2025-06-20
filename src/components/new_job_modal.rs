// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use byos_queuer::queue::Queue;
use dioxus::prelude::*;

use crate::{
    QUEUE,
    components::{
        base_workflow::BaseWorkflow, delete_icon::DeleteIcon, group_samples::GroupSamples,
        modifications_file::ModificationsFile, output_directory::OutputDirectory,
        protein_file::ProteinFile, sample_files::SampleFiles,
    },
};

#[component]
pub fn NewJobModal(id: &'static str) -> Element {
    let base_workflow = use_signal(|| None);
    let sample_files = use_signal(Vec::new);
    let protein_file = use_signal(|| None);
    let modifications_file = use_signal(|| None);
    let output_directory = use_signal(|| None);
    let grouped = use_signal(|| false);

    let mut error_message = use_signal(|| None);

    let close_modal = move || {
        document::eval(&format!("{id}.close()"));
    };

    let onsubmit = move |_| {
        let queue_jobs = if grouped() {
            Queue::queue_grouped_job
        } else {
            Queue::queue_jobs
        };

        // SAFETY: Submission should only be possible if all of the required values are `Some(...)`, so these unwraps
        // should never panic
        let result = queue_jobs(
            &QUEUE.read().unwrap(),
            &base_workflow().unwrap(),
            &sample_files(),
            &protein_file().unwrap(),
            modifications_file().as_ref(),
            &output_directory().unwrap(),
        );

        match result {
            Ok(()) => {
                error_message.set(None);
                close_modal();
            }
            Err(report) => error_message.set(Some(report.to_string())),
        }
    };

    // NOTE: This is something of a hack, since ideally the required file inputs would just be marked `required`, and
    // the form would only allow submission when those requirements are met, but `<input type="file">` is deeply broken
    // in Dioxus at the moment and doesn't seem to acknowledge when files have been uploaded (so validation always
    // fails, even if you've uploaded something)
    let disabled = use_memo(move || {
        base_workflow().is_none()
            || sample_files().is_empty()
            || protein_file().is_none()
            || output_directory().is_none()
    });
    rsx! {
        dialog { class: "modal", id,
            form {
                class: "modal-box overflow-visible flex flex-col items-stretch gap-4",
                method: "dialog",
                onsubmit,

                h3 { class: "text-lg font-bold text-center mb-1", "Queue Job(s)" }

                div { class: "grid grid-cols-[min-content_1fr] gap-y-4",
                    BaseWorkflow { value: base_workflow }
                    SampleFiles { value: sample_files }
                    ProteinFile { value: protein_file }
                    ModificationsFile { value: modifications_file }
                    OutputDirectory { value: output_directory }
                }
                GroupSamples { value: grouped }

                if let Some(error_message) = error_message() {
                    div { role: "alert", class: "alert alert-error",
                        DeleteIcon {}
                        {error_message}
                    }
                }

                div { class: "modal-action mt-2",
                    button {
                        class: "btn grow",
                        r#type: "button",
                        formnovalidate: true,
                        onclick: move |_| close_modal(),
                        "Cancel"
                    }
                    button {
                        class: "btn grow btn-primary",
                        r#type: "submit",
                        disabled,
                        "Queue"
                    }
                }
            }
        }
    }
}
