use dioxus::prelude::*;

#[component]
pub fn Job(name: String) -> Element {
    rsx! {
        li {
            class: "list-row",
            div {
                img {
                    class: "size-10 rounded-box",
                    src: "https://img.daisyui.com/images/profile/demo/1@94.webp"
                }
            }
            div {
                div {
                    { name }
                }
                div {
                    class: "text-xs uppercase font-semibold opacity-60",
                    "Remaining Reason"
                }
            }
        }
    }
}
