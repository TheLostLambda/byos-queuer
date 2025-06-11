watch:
    watchexec -e rs,toml just test lint

test:
    cargo nextest run --retries 4

lint:
    cargo clippy --tests -- -W clippy::nursery -W clippy::pedantic -W clippy::cargo -A clippy::missing_errors_doc -A clippy::missing_panics_doc -A clippy::cargo_common_metadata -A clippy::multiple_crate_versions
    cargo fmt --check
    dx fmt --check

fmt:
    cargo fmt
    dx fmt

tailwatch:
    deno run -A npm:@tailwindcss/cli -i ./input.css -o ./assets/tailwind.css --watch

serve:
    dx serve

xwin:
    cargo xwin build --target x86_64-pc-windows-msvc --release
