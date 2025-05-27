use crate::FAVICON;
use dioxus::prelude::*;

#[component]
pub fn Header() -> Element {
    rsx! {
        div {
            class: "flex items-center justify-center gap-8 mb-8",
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
