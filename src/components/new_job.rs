use dioxus::prelude::*;

use crate::components::new_icon::NewIcon;

#[component]
pub fn NewJob() -> Element {
    rsx! {
        li { class: "list-row",
            button { class: "list-col-grow btn btn-block", NewIcon {} }
        }
    }
}
