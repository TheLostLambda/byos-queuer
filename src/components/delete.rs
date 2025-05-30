use dioxus::prelude::*;

#[component]
pub fn Delete() -> Element {
    rsx! {
        div {
            class: "tooltip",
            "data-tip": "Delete this job",

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
                class: "lucide lucide-circle-x-icon lucide-circle-x",

                circle {
                    cx: "12",
                    cy: "12",
                    r: "10"
                }

                path {
                    d: "m15 9-6 6",
                }

                path {
                    d: "m9 9 6 6",
                }
            }
        }
    }
}
