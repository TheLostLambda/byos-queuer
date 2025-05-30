use dioxus::prelude::*;

#[component]
pub fn Reset() -> Element {
    rsx! {
        div {
            class: "tooltip",
            "data-tip": "Reset this job",

            svg {
                xmlns: "http://www.w3.org/2000/svg",
                width: "24",
                height: "24",
                view_box: "0 0 24 24",
                fill: "none",
                stroke: "currentColor",
                stroke_width: "2",
                stroke_linecap: "round",
                stroke_linejoin: "round",
                class: "lucide lucide-rotate-ccw-icon lucide-rotate-ccw",

                path {
                    d: "M3 12a9 9 0 1 0 9-9 9.75 9.75 0 0 0-6.74 2.74L3 8",
                }

                path {
                    d: "M3 3v5h5",
                }
           }
        }
    }
}
