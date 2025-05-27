mod components;
mod state;

use color_eyre::Result;
use dioxus::{
    desktop::{self, WindowBuilder},
    prelude::*,
};

use components::Header;

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
            class: "card w-9/10 bg-base-100 shadow-sm",

            div {
                class: "flex flex-col card-body",
            }
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
