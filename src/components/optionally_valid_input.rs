use std::str::FromStr;

use dioxus::prelude::*;

#[component]
pub fn OptionallyValidInput<T: Clone + ToString + FromStr + PartialEq + 'static>(
    value: ReadOnlySignal<Option<T>>,
    oninput: EventHandler<Option<T>>,
    #[props(extends = GlobalAttributes, extends = input)] input_attributes: Vec<Attribute>,
) -> Element {
    let mut raw_input = use_signal(String::new);
    use_effect(move || {
        if let Some(value) = value() {
            raw_input.set(value.to_string());
        }
    });

    rsx! {
        input {
            value: raw_input,
            oninput: move |event| {
                raw_input.set(event.value());
                oninput.call(raw_input().parse().ok());
            },
            ..input_attributes,
        }
    }
}
