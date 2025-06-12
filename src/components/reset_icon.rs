use dioxus::prelude::*;

#[component]
pub fn ResetIcon() -> Element {
    rsx! {
        svg {
            class: "lucide lucide-rotate-ccw-icon lucide-rotate-ccw",
            fill: "none",
            height: "24",
            stroke: "currentColor",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            stroke_width: "2",
            view_box: "0 0 24 24",
            width: "24",
            xmlns: "http://www.w3.org/2000/svg",
            path { d: "M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8" }
            path { d: "M3 3v5h5" }
        }
    }
}
