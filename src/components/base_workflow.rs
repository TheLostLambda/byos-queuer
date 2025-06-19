// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;

use crate::components::file_path_picker::FilePathPicker;

#[component]
pub fn BaseWorkflow(value: Signal<Option<String>>) -> Element {
    let files = use_signal(Vec::new);
    use_effect(move || {
        value.set(files().first().cloned());
    });

    rsx! {
        FilePathPicker {
            label: "Base Workflow",
            tooltip: "***TODO***",
            value: files,
            accept: ".wflw",
        }
    }
}
