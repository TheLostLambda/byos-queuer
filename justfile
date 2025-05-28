watch:
    watchexec -e rs,toml cargo nextest run --retries 2

lint:
    cargo clippy --tests -- -W clippy::nursery -W clippy::pedantic -W clippy::cargo -A clippy::missing_errors_doc -A clippy::missing_panics_doc -A clippy::cargo_common_metadata -A clippy::multiple_crate_versions

tailwatch:
    deno run -A npm:@tailwindcss/cli -i ./input.css -o ./assets/tailwind.css --watch

serve:
    dx serve
