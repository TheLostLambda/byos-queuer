use std::time::Duration;

use dioxus::prelude::*;

use byos_queuer::job::Status;

#[derive(Clone)]
pub(super) struct StatusProp(Status);

#[component]
pub fn StatusBadge(status: StatusProp) -> Element {
    let (color_class, content, tooltip) = match status.0 {
        Status::Queued => ("badge-neutral", rsx! { "Queued" }, None),
        Status::Running(_, instant) => {
            let run_time = format_duration(instant.elapsed());
            ("badge-info", rsx! { "Running ({run_time})" }, None)
        }
        Status::Completed(duration) => {
            let run_time = format_duration(duration);
            ("badge-success", rsx! { "Completed ({run_time})" }, None)
        }
        Status::Failed(report, duration) => {
            let run_time = format_duration(duration);
            (
                "badge-warning",
                rsx! { "Failed ({run_time})" },
                Some(report.to_string()),
            )
        }
    };

    rsx! {
        div {
            class: "tooltip tooltip-left badge {color_class} font-mono",

            div {
                class: "tooltip-content",
                { tooltip }
            }

            { content }
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

fn format_duration(duration: Duration) -> String {
    let elapsed_seconds = duration.as_secs();
    let seconds = elapsed_seconds % 60;
    let minutes = (elapsed_seconds / 60) % 60;
    let hours = (elapsed_seconds / 60) / 60;

    let hours = if hours != 0 {
        format!("{hours}h")
    } else {
        String::new()
    };
    let minutes = if minutes != 0 {
        format!("{minutes}m")
    } else {
        String::new()
    };

    format!("{hours}{minutes}{seconds}s")
}
