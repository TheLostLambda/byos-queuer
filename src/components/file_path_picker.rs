use std::{ffi::OsStr, path::Path};

use dioxus::prelude::*;

#[component]
pub fn FilePathPicker(
    label: &'static str,
    tooltip: &'static str,
    value: Signal<Vec<String>>,
    #[props(default = false)] directory: bool,
    #[props(default = false)] multiple: bool,
    #[props(extends = GlobalAttributes, extends = input)] input_attributes: Vec<Attribute>,
) -> Element {
    let upload_noun = match (directory, multiple) {
        (false, false) => "file",
        (false, true) => "files",
        (true, false) => "directory",
        (true, true) => "directories",
    };

    let input_text = use_memo(move || {
        if value().is_empty() {
            format!("Click to select {upload_noun}...")
        } else {
            let path_count = value().len();
            let file_names = value()
                .iter()
                .map(|path_string| {
                    Path::new(path_string)
                        .file_name()
                        .and_then(OsStr::to_str)
                        .unwrap_or(path_string)
                })
                .collect::<Vec<_>>()
                .join(", ");

            if path_count > 1 {
                format!("{path_count} {upload_noun} — {file_names}")
            } else {
                file_names
            }
        }
    });

    rsx! {
        label { class: "col-span-2 grid grid-cols-subgrid input w-full",
            span { class: "label tooltip",
                {label}

                p { class: "tooltip-content", {tooltip} }
            }
            label { class: "cursor-pointer truncate",
                {input_text}
                input {
                    class: "absolute h-0 w-0 p-0 opacity-0",
                    onchange: move |event| {
                        // TODO: This could perhaps be done better (by actually using the `FileData`
                        // objects provided by Dioxus), but just converting the `PathBuf`s to
                        // `String`s was the quickest way to migrate from Dioxus 0.6 to 0.7
                        *value.write() = event
                            .files()
                            .into_iter()
                            .map(|f| f.path().into_os_string().into_string().unwrap())
                            .collect();
                    },
                    r#type: "file",
                    directory,
                    multiple,
                    ..input_attributes,
                }
            }
        }
    }
}
