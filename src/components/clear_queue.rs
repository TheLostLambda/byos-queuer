use dioxus::prelude::*;

use crate::{QUEUE, components::delete_icon::DeleteIcon};

#[component]
pub fn ClearQueue() -> Element {
    rsx! {
        div {
            class: "tooltip",
            "data-tip": "Clear whole queue",
            onclick: move |_| QUEUE.read().unwrap().clear().unwrap(),

            DeleteIcon {}
        }
    }
}
