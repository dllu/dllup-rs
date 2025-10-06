dllup markup language

===

a simple markup language for personal blog

* has first class support for math equations that get rendered to svg images. your static webpage won't jump around as stuff are rendering/drawing.

## Configuration

Run the binary with `dllup-rs <input.dllu> [config.toml]`. If a config path is not provided, the tool looks for `dllup.toml` next to the input file. Missing config files fall back to built-in defaults.

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

[math]
# Try to spawn the persistent Node.js-based KaTeX helper before other options
prefer_persistent = false

# External command used to render math when present. The command should read
# TeX from stdin and write HTML to stdout, matching KaTeX CLI behaviour.
command = "npx katex"
```

Math is rendered to inline HTML (KaTeX-compatible). When `math.command` is set the tool will run it, otherwise it first tries to spawn the bundled persistent KaTeX helper and falls back to `npx katex`. If every option fails, the raw TeX is emitted inside `<span class="math-inline">` or `<div class="math-display">` elements.

When `root_url` is configured, any link or image whose URL starts with `/` is prefixed with that root (e.g., `/foo.html` becomes `https://example.com/foo.html`). The configured `css_href` follows the same rules when it is relative.

The template is rendered by replacing `{{title}}`, `{{css}}`, `{{tableofcontents}}`, `{{metas}}`, and `{{body}}`. A nested table of contents is generated from the section headings; include `{{tableofcontents}}` inside the template to display it. The `{{metas}}` placeholder is populated with Open Graph / Twitter tags derived from the first paragraph and first image, along with sensible defaults for robots and card type.
* supports cross references references and tables
* html5 semantic figure and figcaption for images
* implemented in rust for some reason
