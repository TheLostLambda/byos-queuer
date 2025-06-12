use dioxus::prelude::*;

use crate::components::{clear_queue::ClearQueue, reset_queue::ResetQueue};

#[component]
pub fn TopBar() -> Element {
    rsx! {
        div { class: "flex items-center justify-between gap-4 px-4",
            h2 { class: "card-title grow", "Queued Jobs" }

            ResetQueue {}
            ClearQueue {}
        }
    }
}
