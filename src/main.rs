mod components;
mod state;

use std::{fs, time::Duration};

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
    color_eyre::install()?;

    // FIXME: Get rid of all of this once it's not longer needed for testing!
    const BASE_WORKFLOW: &str = "tests/data/PG Monomers.wflw";
    const SAMPLE_FILES: [&str; 2] = ["tests/data/WT.raw", "tests/data/6ldt.raw"];
    const PROTEIN_FILE: &str = "tests/data/proteins.fasta";
    const MODIFICATIONS_FILE: Option<&str> = Some("tests/data/modifications.txt");
    const OUTPUT_DIRECTORY: &str = "/home/tll/Downloads/byos-queuer/";

    let _ = fs::remove_dir_all(OUTPUT_DIRECTORY);
    fs::create_dir(OUTPUT_DIRECTORY).unwrap();

    let queue = Queue::new(2, Duration::from_millis(1000)).unwrap();

    queue
        .queue(
            BASE_WORKFLOW,
            SAMPLE_FILES,
            PROTEIN_FILE,
            MODIFICATIONS_FILE,
            OUTPUT_DIRECTORY,
        )
        .unwrap();

    queue
        .queue_grouped(
            BASE_WORKFLOW,
            SAMPLE_FILES,
            PROTEIN_FILE,
            MODIFICATIONS_FILE,
            OUTPUT_DIRECTORY,
        )
        .unwrap();

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
