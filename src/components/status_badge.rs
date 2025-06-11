use dioxus::prelude::*;

use byos_queuer::job::Status;

use crate::components::run_timer::RunTime;

#[derive(Clone)]
pub(super) struct StatusProp(Status);

#[component]
pub fn StatusBadge(status: StatusProp) -> Element {
    let (color_class, content, tooltip) = match status.0 {
        Status::Queued => ("badge-neutral", rsx! { "Queued" }, None),
        Status::Running(_, instant) => (
            "badge-primary",
            rsx! {
                "Running "
                RunTime { time: instant }
            },
            None,
        ),
        Status::Completed(duration) => (
            "badge-success",
            rsx! {
                "Completed "
                RunTime { time: duration }
            },
            None,
        ),
        Status::Failed(report, duration) => (
            "badge-error",
            rsx! {
                "Failed "
                RunTime { time: duration }
            },
            Some(report.to_string()),
        ),
        Status::Resetting => ("badge-warning", rsx! { "Stopping..." }, None),
        Status::Abandoned => unreachable!(),
    };

    rsx! {
        div { class: "tooltip tooltip-left badge {color_class} font-mono",

            div { class: "tooltip-content", {tooltip} }

            {content}
        }
    }
}

impl From<Status> for StatusProp {
    fn from(value: Status) -> Self {
        Self(value)
    }
}

// NOTE: The `Running` and `Failed` statuses contain fields that cannot be tested for equality, so this implementation
// simply ignores those fields. This means that statuses can be `PartialEq` whilst actually being different values! To
// avoid introducing that "buggy" behaviour in the public `job::Status` struct, I'm implementing it for this private
// `StatusProps` wrapper instead
impl PartialEq for StatusProp {
    fn eq(&self, other: &Self) -> bool {
        // NOTE: Saves me needing to retype `Status::` a million times!
        use Status::*;

        // NOTE: It would be really nice if I could cut down on the repetition and group all of this into one call to
        // `matches!(..)`, but that's blocked on https://github.com/rust-lang/rust/issues/129967
        match (&self.0, &other.0) {
            (Queued, Queued) => true,
            (Running(_, i1), Running(_, i2)) => i1 == i2,
            (Completed(d1), Completed(d2)) | (Failed(_, d1), Failed(_, d2)) => d1 == d2,
            _ => false,
        }
    }
}
