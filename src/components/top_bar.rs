use dioxus::prelude::*;

use crate::components::{delete_icon::DeleteIcon, reset_icon::ResetIcon};

#[component]
pub fn TopBar() -> Element {
    rsx! {
        div { class: "flex items-center justify-between gap-4 px-4",
            h2 { class: "card-title grow", "Queued Jobs" }

            ResetIcon {}
            DeleteIcon {}
        }
    }
}
