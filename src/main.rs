mod components;
mod state;

use std::{
    env, path,
    sync::{LazyLock, RwLock},
    time::Duration,
};

use byos_queuer::queue::Queue;
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

        Header {}

        main { class: "card w-full bg-base-100 shadow-sm", JobQueue {} }
    }
}

fn main() -> Result<()> {
    // FIXME: Kill this block!

    const TEST_PATH: &str = "tests/scripts/queue-ui";
    let old_path = env::var("PATH").unwrap_or_default();
    let new_path = path::absolute(TEST_PATH).unwrap();
    let joined_path = format!("{new_path}:{old_path}", new_path = new_path.display());

    unsafe {
        env::set_var("PATH", joined_path);
    }

    // FIXME: End of block to kill

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
