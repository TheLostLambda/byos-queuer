use crate::FAVICON;
use dioxus::prelude::*;

#[component]
pub fn Header() -> Element {
    rsx! {
        // We can create elements inside the rsx macro with the element name followed by a block of attributes and children.
        div {
            // Attributes should be defined in the element before any children
            class: "flex items-center justify-center gap-8 mb-8",
            // After all attributes are defined, we can define child elements and components
            img {
                src: FAVICON,
                class: "h-24",
            },
            h1 {
                class: "text-6xl font-mono font-bold",
                "Byos Queuer"
            }
        }
    }
}
