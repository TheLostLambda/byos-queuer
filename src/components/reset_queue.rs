use dioxus::prelude::*;

use crate::{QUEUE, components::reset_icon::ResetIcon};

#[component]
pub fn ResetQueue() -> Element {
    rsx! {
        div {
            class: "tooltip",
            "data-tip": "Reset whole queue",
            onclick: move |_| QUEUE.read().unwrap().reset().unwrap(),

            ResetIcon {}
        }
    }
}
