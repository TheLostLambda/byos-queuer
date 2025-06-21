// NOTE: This hides the terminal that pops up on Windows when the application is run
#![windows_subsystem = "windows"]

mod components;

use std::{
    sync::{LazyLock, RwLock},
    time::Duration,
};

use byos_queuer::Queue;
use color_eyre::Result;
use dioxus::{
    desktop::{self, WindowBuilder},
    prelude::*,
};

use components::{Header, JobQueue};

pub const FAVICON: Asset = asset!("/assets/favicon.ico");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

const INDEX_HTML: &str = include_str!("../index.html");

const DEFAULT_WORKERS: usize = 6;
const DEFAULT_STAGGER_DURATION: Duration = Duration::from_secs(3);

pub static QUEUE: LazyLock<RwLock<Queue>> =
    LazyLock::new(|| RwLock::new(Queue::new(DEFAULT_WORKERS, DEFAULT_STAGGER_DURATION).unwrap()));

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Stylesheet { href: TAILWIND_CSS }

        main { class: "flex flex-col items-center w-full min-w-137",
            Header {}
            div { class: "card w-256 max-w-full bg-base-100 shadow-sm", JobQueue {} }
        }
    }
}

fn main() -> Result<()> {
    color_eyre::install()?;

    dioxus::LaunchBuilder::new()
        .with_cfg(
            desktop::Config::default()
                .with_menu(None)
                .with_window(WindowBuilder::new().with_title("Byos Queuer"))
                .with_custom_index(INDEX_HTML.to_string()),
        )
        .launch(App);

    Ok(())
}
