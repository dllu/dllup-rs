dllup markup language

===

a simple markup language for personal blog

* has first class support for math equations that get rendered to svg images. your static webpage won't jump around as stuff are rendering/drawing.

## Math rendering

Math is rendered to inline HTML (e.g., KaTeX output) and inlined directly in the document.

- External command engine (recommended):
  - Set `DLLUP_KATEX_CMD` (or `DLLUP_MATH_CMD`) to a command that prints rendered HTML to stdout.
  - The command receives a flag for mode and the TeX string as the final argument:
    - `--inline` for inline math
    - `--display` for display math
    - The TeX string is passed as the final argument (with a leading space to avoid being parsed as an option if it begins with `-`).
  - Example: `DLLUP_KATEX_CMD='deno run -A ./deno/katex_render.ts'`

- Fallback:
  - If no command is configured or rendering fails, the raw TeX is shown inside `<span class="math-inline">` or `<div class="math-display">`.
* supports cross references references and tables
* html5 semantic figure and figcaption for images
* implemented in rust for some reason
