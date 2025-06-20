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
                .filter_map(|path_string| {
                    Path::new(path_string).file_name().and_then(OsStr::to_str)
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
                    class: "h-0 w-0 p-0 opacity-0",
                    onchange: move |event| {
                        *value.write() = event.files().unwrap().files();
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
