// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;

use crate::components::file_path_picker::FilePathPicker;

#[component]
pub fn ModificationsFile(value: Signal<Option<String>>) -> Element {
    let paths = use_signal(Vec::new);
    use_effect(move || {
        value.set(paths().first().cloned());
    });

    rsx! {
        FilePathPicker {
            label: "Modifications File",
            tooltip: "A .txt file containing Byos-compatible \"Custom modification text\"",
            value: paths,
            accept: ".txt",
        }
    }
}
