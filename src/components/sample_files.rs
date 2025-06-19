// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;

use crate::components::file_path_picker::FilePathPicker;

#[component]
pub fn SampleFiles(value: Signal<Vec<String>>) -> Element {
    rsx! {
        FilePathPicker {
            label: "Sample Files",
            tooltip: "***TODO***",
            value,
            // TODO: Extend this `accept` to allow more than just Thermo RAW files? I'll first have to find out what
            // other file / directory extensions Byos supports...
            accept: ".raw",
            multiple: true,
        }
    }
}
