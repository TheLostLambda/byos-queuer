use dioxus::prelude::*;

#[component]
pub fn LaunchInterval(
    value: ReadOnlySignal<Option<u64>>,
    oninput: EventHandler<Option<u64>>,
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
                "Launch Interval"

                p { class: "tooltip-content",
                    "If several jobs are started at the same time, then there is a chance that some of them will \
                        spontaneously fail. These failures are caused by different Byos instances clashing during \
                        startup and can be resolved by staggering the launch of new jobs by some (empirically \
                        determined) number of seconds."
                }
            }
            input {
                value: raw_input,
                oninput: move |event| {
                    raw_input.set(event.value());
                    oninput.call(raw_input().parse().ok());
                },
                min: "0",
                r#type: "number",
                required: "true",
            }
            span { class: "label",
                if value() == Some(1) {
                    "second"
                } else {
                    "seconds"
                }
            }
        }
    }
}
