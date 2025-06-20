use dioxus::prelude::*;

#[component]
pub fn MaximumConcurrentJobs(
    value: ReadOnlySignal<Option<usize>>,
    oninput: EventHandler<Option<usize>>,
) -> Element {
    let mut raw_input = use_signal(String::new);
    use_effect(move || {
        if let Some(value) = value() {
            raw_input.set(value.to_string());
        }
    });

    rsx! {
        label { class: "col-span-3 grid grid-cols-subgrid input w-full",
            span { class: "label tooltip",
                "Maximum Concurrent Jobs"

                p { class: "tooltip-content",
                    "Limits the number of jobs that can be run at once. Increasing this number can lead to higher \
                        throughput (more jobs completed per unit of time), but individual jobs will take longer to \
                        complete, and — if this parameter is set too high — the computer could run out of memory."
                }
            }
            input {
                class: "col-span-2",
                value: raw_input,
                oninput: move |event| {
                    raw_input.set(event.value());
                    oninput.call(raw_input().parse().ok());
                },
                min: "1",
                r#type: "number",
                required: "true",
            }
        }
    }
}
