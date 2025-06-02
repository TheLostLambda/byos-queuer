use std::path::PathBuf;

use dioxus::signals::Signal;

// FIXME: Make sure this is either removed or used!
#[expect(dead_code)]
#[derive(Clone, Default)]
struct State {
    base_workflow: Signal<PathBuf>,
    modifications_file: Signal<Option<PathBuf>>,
    protein_file: Signal<Option<PathBuf>>,
    input_files: Signal<Vec<PathBuf>>,
    output_directory: Signal<Option<PathBuf>>,
}
