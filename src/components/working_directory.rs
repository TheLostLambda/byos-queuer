// NOTE: The `#[component]` macro is deriving `PartialEq`, but not `Eq` (since that's not needed), and clippy is
// complaining about that. This needs to be a module-level `#![expect(...)]` since I can't actually place an
// `#[expect(...)]` inside of the `#[component]` macro
#![expect(clippy::derive_partial_eq_without_eq)]

use dioxus::prelude::*;

use crate::components::file_path_picker::FilePathPicker;

#[component]
pub fn WorkingDirectory(value: Signal<Option<String>>) -> Element {
    let paths = use_signal(Vec::new);
    use_effect(move || {
        value.set(paths().first().cloned());
    });

    rsx! {
        FilePathPicker {
            label: "Working Directory",
            tooltip: "An optional directory to run Byos searches in before copying results to the output directory. \
                      Some workflows, especially those generating FTRS files, can be bottlenecked by disk I/O, so \
                      picking a working directory on an SSD or RAM disk can result in a significant speedup",
            value: paths,
            directory: true,
        }
    }
}
