This is a Rust project that renders Markdown into the custom HTML used by daniel.lawrence.lu. It is a static site generator with a custom renderer, not a bespoke markup-language parser.

Input files are `.md` Markdown files. Keep syntax and docs Markdown-oriented unless a small renderer convention is explicitly needed, such as YAML front matter, `Table:` captions, `::` big buttons, math, or image-only paragraphs rendered as figures.

After making changes to the code, run `cargo check`, `cargo test`, and `cargo clippy --all-targets -- -D warnings` when practical. You can run `cargo run -- example dllup.toml` to compile the example website.
