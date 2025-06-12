use dioxus::prelude::*;

use crate::{STATE, components::reset_icon::ResetIcon};

#[component]
pub fn ResetQueue() -> Element {
    rsx! {
        div {
            class: "tooltip",
            "data-tip": "Reset whole queue",
            onclick: move |_| STATE.read().unwrap().reset().unwrap(),

            ResetIcon {}
        }
    }
}
