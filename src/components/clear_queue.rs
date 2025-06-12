use dioxus::prelude::*;

use crate::{STATE, components::delete_icon::DeleteIcon};

#[component]
pub fn ClearQueue() -> Element {
    rsx! {
        div {
            class: "tooltip",
            "data-tip": "Clear whole queue",
            onclick: move |_| STATE.read().unwrap().clear().unwrap(),

            DeleteIcon {}
        }
    }
}
