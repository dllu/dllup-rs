# dllup-rs

A custom Markdown renderer and static site generator for
[daniel.lawrence.lu](https://daniel.lawrence.lu).

The input format is Markdown, parsed with `markdown-rs`. The renderer is custom
because the site needs opinionated handling for photos, math, tables, blog
indexes, feeds, and metadata.

* renders math with KaTeX HTML during site generation, so pages stay static with
  no client-side math rendering.
* precomputes image dimensions, so lazily loaded photos do not cause layout
  shifts.
* generates thumbnails, includes EXIF metadata in captions, and handles large
  photo libraries with parallel image resizing.
* generates blog indexes, RSS feeds, sitemaps, Open Graph tags, and a nested
  table of contents.

## markdown conventions

Most content is ordinary Markdown. The renderer adds a few site-specific
conventions:

* YAML front matter supplies page metadata:

  ```md
  ---
  title: Page title
  date: 2026-01-31
  ---
  ```

* Image- or video-only paragraphs become captioned figures. The image label is
  used as the visible caption, and as alt text for images:

  ```md
  ![Purple Puppy driving a Porsche 356A Speedster](porsche.svg)
  ![A short demonstration clip](demo.mp4)
  ```

* GitHub-flavored Markdown tables can be followed by a separate `Table:`
  paragraph to render a table caption.
* Inline math uses `$...$`; display math uses `$$` fences.
* Raw HTML is passed through for pages that need custom demos or embeds.
* Big buttons use the small renderer extension `:: Label https://example.com`.

See `docs/markdown-migration.md` and `example/blog/doc/index.md` for more
syntax notes.

## config

Run the binary with `dllup-rs <input.md|directory> [config.toml]`. If a config
path is not provided, the tool looks for `dllup.toml` next to the input file.
Missing config files fall back to built-in defaults.

For example:

```sh
cargo run -- example/index.md dllup.toml
cargo run -- example dllup.toml
```

All settings live inside the TOML file. Available keys:

```toml
# Enable timing output on stderr
timings = false

# Base URL used for site-relative links like "/post.html"
root_url = "https://example.com"

# Path to the HTML wrapper template used to produce the final page
template_path = "static/template.html"

# HREF for the page stylesheet. Relative values are joined with root_url.
css_href = "static/styles.css"

[images]
# Directory where downloaded originals and generated variants are cached
cache_dir = "img"

# Optional CDN base URL used for image links (falls back to root_url)
# img_root_url = "https://cdn.example.com/images"

# Responsive widths (in pixels) generated for raster images
sizes = [480, 800, 1200]

# Subset of `sizes` used to populate the <img> srcset attribute
display_sizes = [480, 800]

# Width used to pre-compute layout dimensions when metadata is missing
layout_width = 1200

# JPEG quality for resized outputs
jpeg_quality = 85

# Timeout for downloading remote images before falling back to the original URL
remote_fetch_timeout_secs = 10

[math]
# Try to spawn the persistent Node.js-based KaTeX helper before other options
prefer_persistent = false

# External command used to render math when present. The command should read
# TeX from stdin and write HTML to stdout, matching KaTeX CLI behaviour.
command = "npx katex"
```

Math is rendered to inline HTML (KaTeX-compatible). When `math.command` is set the tool will run it, otherwise it first tries to spawn the bundled persistent KaTeX helper and falls back to `npx katex`. If every option fails, the raw TeX is emitted inside `<span class="math-inline">` or `<div class="math-display">` elements.

When `root_url` is configured, any link or image whose URL starts with `/` is prefixed with that root (e.g., `/foo.html` becomes `https://example.com/foo.html`). The configured `css_href` follows the same rules when it is relative. Image assets can opt into a dedicated CDN by setting `images.img_root_url`; when omitted, `root_url` continues to be used.

The template is rendered by replacing `{{title}}`, `{{css}}`, `{{tableofcontents}}`, `{{metas}}`, and `{{body}}`. A nested table of contents is generated from the section headings; include `{{tableofcontents}}` inside the template to display it. The `{{metas}}` placeholder is populated with Open Graph / Twitter tags derived from the first paragraph and first image, along with sensible defaults for robots and card type.

## development

After changes, run:

```sh
cargo check
cargo test
cargo clippy --all-targets -- -D warnings
cargo run -- example dllup.toml
```
