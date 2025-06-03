mod components;
mod state;

use std::{
    env, fs, path,
    sync::{LazyLock, RwLock},
    time::Duration,
};

use byos_queuer::queue::Queue;
use color_eyre::Result;
use dioxus::{
    desktop::{self, WindowBuilder},
    prelude::*,
};

use components::{Header, Jobs};

pub const FAVICON: Asset = asset!("/assets/favicon.ico");
const TAILWIND_CSS: Asset = asset!("/assets/tailwind.css");

const INDEX_HTML: &str = include_str!("../index.html");

// FIXME: Get rid of all of this once it's not longer needed for testing!
const BASE_WORKFLOW: &str = "tests/data/PG Monomers.wflw";
const SAMPLE_FILES: [&str; 2] = ["tests/data/WT.raw", "tests/data/6ldt.raw"];
const PROTEIN_FILE: &str = "tests/data/proteins.fasta";
const MODIFICATIONS_FILE: Option<&str> = Some("tests/data/modifications.txt");
const OUTPUT_DIRECTORY: &str = "/home/tll/Downloads/byos-queuer/";

pub static STATE: LazyLock<RwLock<Queue>> = LazyLock::new(|| {
    let _ = fs::remove_dir_all(OUTPUT_DIRECTORY);
    fs::create_dir(OUTPUT_DIRECTORY).unwrap();

    let queue = Queue::new(2, Duration::from_millis(1000)).unwrap();

    queue
        .queue_jobs(
            BASE_WORKFLOW,
            SAMPLE_FILES,
            PROTEIN_FILE,
            MODIFICATIONS_FILE,
            OUTPUT_DIRECTORY,
        )
        .unwrap();

    queue
        .queue_grouped_job(
            BASE_WORKFLOW,
            SAMPLE_FILES,
            PROTEIN_FILE,
            MODIFICATIONS_FILE,
            OUTPUT_DIRECTORY,
        )
        .unwrap();

    RwLock::new(queue)
});

#[component]
fn App() -> Element {
    rsx! {
        document::Link { rel: "icon", href: FAVICON }
        document::Stylesheet { href: TAILWIND_CSS }

        Header {},

        main {
            class: "card w-full bg-base-100 shadow-sm",

            div {
                class: "flex flex-col card-body",
                Jobs {}
            }
        }
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

    STATE.read().unwrap().run().unwrap();

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
