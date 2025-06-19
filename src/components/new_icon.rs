use dioxus::prelude::*;

#[component]
pub fn NewIcon() -> Element {
    rsx! {
        svg {
            class: "lucide lucide-circle-plus-icon lucide-circle-plus",
            fill: "none",
            height: "24",
            stroke: "currentColor",
            stroke_linecap: "round",
            stroke_linejoin: "round",
            stroke_width: "2",
            view_box: "0 0 24 24",
            width: "24",
            xmlns: "http://www.w3.org/2000/svg",
            circle { cx: "12", cy: "12", r: "10" }
            path { d: "M8 12h8" }
            path { d: "M12 8v8" }
        }
    }
}
