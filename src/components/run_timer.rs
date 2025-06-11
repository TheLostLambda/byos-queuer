use std::time::{Duration, Instant};

use dioxus::prelude::*;
use tokio::time::sleep;

#[derive(Copy, Clone, PartialEq)]
pub(super) enum InstantOrDuration {
    Instant(Instant),
    Duration(Duration),
}

#[component]
pub fn RunTime(#[props(into)] time: InstantOrDuration) -> Element {
    use_coroutine(move |_: UnboundedReceiver<()>| async move {
        if let InstantOrDuration::Instant(instant) = time {
            loop {
                let duration_until_next_second =
                    Duration::from_millis(1000 - u64::from(instant.elapsed().subsec_millis()));
                sleep(duration_until_next_second).await;
                needs_update();
            }
        }
    });

    let duration = match time {
        InstantOrDuration::Instant(instant) => instant.elapsed(),
        InstantOrDuration::Duration(duration) => duration,
    };
    let run_time = format_duration(duration);

    rsx! { "({run_time})" }
}

impl From<Instant> for InstantOrDuration {
    fn from(value: Instant) -> Self {
        Self::Instant(value)
    }
}

impl From<Duration> for InstantOrDuration {
    fn from(value: Duration) -> Self {
        Self::Duration(value)
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
