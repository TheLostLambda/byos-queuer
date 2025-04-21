use std::path::PathBuf;

use dioxus::signals::Signal;

#[derive(Clone, Default)]
struct WorkflowSettings {
    base_workflow: Signal<PathBuf>,
    modifications_file: Signal<Option<PathBuf>>,
    protein_file: Signal<Option<PathBuf>>,
    input_files: Signal<Vec<PathBuf>>,
    output_directory: Signal<Option<PathBuf>>,
}
