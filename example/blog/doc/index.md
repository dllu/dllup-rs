---
title: "dllup markup language"
date: "2015-03-12"
---


# Introduction

**dllup** is a [lightweight markup language](https://en.wikipedia.org/wiki/Lightweight_markup_language) designed for creating simple websites. It is now mostly Markdown, with a few site-specific conventions that the dllup renderer turns into static HTML pages. This document covers the Markdown-facing syntax used to produce HTML webpages.

The name "dllup" is a portmanteau of my username and "markup". It was inspired by [jemdoc](http://jemdoc.jaboc.net/) and [Markdown](http://daringfireball.net/projects/markdown/). It supports LaTeX equations, tables, figure captioning, syntax highlighting of source code, and automatic section numbering with table of contents. However, it lacks many other features as it is deliberately simple.

## Philosophy

- The raw text file must be fast to type up.
- The output must be semantically correct HTML5 and be parseable by text-only browsers like [Lynx](https://en.wikipedia.org/wiki/Lynx_%28web_browser%29) and accessibility tools. Users can apply whatever CSS styles as they wish.
- In descending order of importance: it should be _simple_, _consistent_, _correct_. Compare with the ["worse is better" approach](http://en.wikipedia.org/wiki/Worse_is_better).

## Comparison to other languages

Why make [yet another markup language](http://xkcd.com/927/) when there are already so many available? The answer is:

1. Just for fun.
2. I want several features not available in current markup languages, and I don't need several features which are available in current markup languages.

Thus, this project is for my own personal use. That notwithstanding, here are comparisons to jemdoc and Markdown.

### Comparison to Markdown

There are several small differences:

- **dllup uses a Markdown parser but a custom renderer**. Most basic Markdown works, but the renderer maps certain Markdown nodes into the site's existing figures, equations, tables, and buttons.
- **dllup supports math equations**. Many attempts exist to bolt on [MathJax](http://www.mathjax.org) support to Markdown (see: [MathJax in use](http://docs.mathjax.org/en/latest/misc/mathjax-in-use.html)). I have made the decision instead to render math as SVG on the serverside.
  There are pros and cons: the good thing is that this avoids the lengthy render time of MathJax, allows repeated equations to be cached, and works on browsers without Javascript. The bad thing is that it takes a bit more bandwidth to send a large SVG file (a few kB per equation) and there is no subpixel hinting.
- **dllup supports captioned figures**. Image-only paragraphs render as semantically correct HTML5 `<figure>` and `<figcaption>` elements.
- **dllup keeps a few local conventions**, such as `Table:` captions and `::` big buttons.

The syntax for headers, links, lists, numbered lists, emphasis, strong text, code fences, tables, and images is valid Markdown.

### Comparison to Jemdoc

There are several differences:

- **dllup equations are SVG**. The equations in jemdoc were originally rasterized PNG images which wastes bandwidth, and looks horrible on retina screens, in print, or when zoomed in. But it should be noted there is a version of jemdoc which uses MathJax instead, with the same pros and cons as described in the previous section.
- **dllup supports captioned figures**. While jemdoc supports image blocks, it does not directly support the semantically correct `<figure>` caption since it targets XHTML 1.1 instead of HTML5.
- **dllup does not implement several features**. This includes other character sets, and so on. There are also no bibliographic reference packages for dllup.
- **dllup is not as portable**, as it requires external software to handle SVG equations.

There is a key similarity: the goal of maintaining personal webpages, especially in an academic setting, is identical.

# Syntax

## Article header

Use YAML frontmatter for the page title and optional date.

```
---
title: "dllup markup language"
date: "2015-03-12"
---

# Introduction
 
**dllup** is a lightweight markup language...
```

## Code

### Raw HTML

Raw HTML can be written directly as Markdown raw HTML.

```
Blah blah.

<script>
window.alert('hi');
</script>

Blah blah blah.
```

### Code blocks

A code block uses a Markdown code fence. Put the language in the fence info string when syntax highlighting should be applied.

````
Blah blah.

```c++
#include <iostream>
int main() {
    std::cout << "Hello world!" << endl;
}
```

Blah blah blah.
````

For example,

```c++
#include <iostream>
int main() {
    std::cout << "Hello world!" << endl;
}
```

## Block elements

All block elements are separated by at least one empty line. For example there must be an empty line between a header and the paragraph that follows it. This behaviour is similar to LaTeX and allows you to use hard wraps (i.e. line breaks) to enforce a maximum line length.

### Section headers

Headers are specified by preceding octothorpes, up to six. These will show up in the Table of Contents, which appears in the article header. Using more than three levels of section headers is not recommended.

```
# This is a first level header

## This is a second level header

### This is a third level header
that is very, very long

###### This is a sixth level header
```

### Blockquotes

Blockquotes are preceded by a greater than symbol.

```
> The quick brown fox jumps over the lazy dog and
also jumps over the lazy cat.
```

This gets rendered as:

> The quick brown fox jumps over the lazy dog and
also jumps over the lazy cat.

### Images

Image-only paragraphs are captioned figures by default. Use standard Markdown image syntax. The image label becomes both the alt text and the figure caption.

```
![_Ghost_ is a novelty chess set I designed which looks cool but is totally impractical for playing. The pieces are very flat and can be stacked together for compact storage.](http://i.imgur.com/WpEUM8S.jpg)
```

This gets rendered as:

![_Ghost_ is a novelty chess set I designed which looks cool but is totally impractical for playing. The pieces are very flat and can be stacked together for compact storage.](http://i.imgur.com/WpEUM8S.jpg)

### Display math equations

Display math equations use double dollar fences.

```
$$
x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}
$$
```

This gets rendered as:

$$
x = \frac{-b \pm \sqrt{b^2 - 4ac}}{2a}
$$

### Lists

Lists use standard Markdown bullets, indentation, and ordered list markers.

```
- Item one.
- Item two.
  Multi-line list item.
  - Nested.
    - More nested.
  - Nested.
- Item 3.

1. Item one
99. Item ninety-nine? Nah it's actually item two.
```

This gets rendered as:

- Item one.
- Item two.
  Multi-line list item.
  - Nested.
    - More nested.
  - Nested.
- Item 3.

1. Item one
99. Item ninety-nine? Nah it's actually item two.

The first number of a numbered list should be 1, although for the remaining list items any number works.

### Tables

Tables use GitHub-flavored Markdown table syntax. A following paragraph beginning with `Table:` becomes the caption.

```
| Puppy colour | Awesome? | Purpleness |
| ------------ | -------- | ---------- |
| Red          | $\times$ | 0.1        |
| Orange       | $\times$ | 0          |
| Yellow       | $\times$ | 0          |
| Green        | $\times$ | 0          |
| Blue         | $\times$ | 0.5        |
| Purple       | $\checkmark$ | 1      |

Table: Summary of the awesomeness of puppy colours.
```

This gets rendered as:

| Puppy colour | Awesome? | Purpleness |
| --- | --- | --- |
| Red | $\times$ | 0.1 |
| Orange | $\times$ | 0 |
| Yellow | $\times$ | 0 |
| Green | $\times$ | 0 |
| Blue | $\times$ | 0.5 |
| Purple | $\checkmark$ | 1 |

Table: Summary of the awesomeness of puppy colours.

The first line is interpreted as a header. The vertical pipes do not need to line up and extra whitespace is discarded.

### Big button

A big button is useful in case you want to link to an important page, or a link to download something, or to perform a critical action.

```
:: big button http://www.dllu.net/
```

This gets rendered as:

:: big button http://www.dllu.net/

### Paragraph

A block with none of the keywords/symbols in front counts as just a paragraph.

## Inline elements

Inline elements, or _span_ elements, apply to any kind of normal text such as paragraph text, header text, caption text, list content, and so on. But they do not apply to math equations or source code. Here are the types of elements processed in order of precedence.

### Monospace code snippet

A `monospace code snippet` comes between backticks.

```
A `monospace code snippet` comes between backticks.
```

### Inline math

Inline mathematics $y = mx + b$ comes between dollar signs.

```
Inline mathematics $y = mx + b$ comes between dollar signs.
```

### Links

Links follow [the Markdown syntax](http://daringfireball.net/projects/markdown/basics).

```
Links follow [the Markdown syntax](http://daringfireball.net/projects/markdown/basics).
```

### Emphasis and strong

Text is _emphasized_ or made **strong** by underscores and double asterisks.

```
Text is _emphasized_ or made **strong** by underscores and double asterisks.
```

### References

Referring to or citing things can be done with ordinary Markdown links to anchors, such as `[Blanco](#blanco)`. A reference list can be written as a regular unordered list with anchors:

- <span id="asdf"></span> Lu, Daniel. "dllup markup language".

```
Referring to or citing things can be done with [links](#asdf).

- <span id="asdf"></span> Lu, Daniel. "dllup markup language".
```

## Typographical character substitutions

dllup makes automatic substitutions:

- Real quotation marks that look like "66" and "99" instead of straight quotation marks.
- Real single quotation marks and apostrophes that look like '6' and '9'.
- Dashes --- em dashes for breaking sentences, and en dashes for ranges like A--Z.
- Ellipses...

### Special character escaping

You can escape special characters using backslashes.

The dollar sign "\$", backtick "\`", for example, should be escaped as `\$` and `\`` respectively if you wish to use them in prose.

# Implementation

dllup is implemented in [Rust](https://www.rust-lang.org/). The parser uses a Markdown AST and maps it into the existing site renderer.

For rendering math equations, dllup uses KaTeX to create SVG output.

For syntax highlighting, dllup uses [inkjet](https://crates.io/crates/inkjet).

The source code of dllup-rs is available on [GitHub](https://github.com/dllu/dllup-rs) under the MIT license.

# Installation

## dllup to HTML

Using dllup is straightforward, but it is not as portable as plain Markdown because the renderer also processes math and images.

- Install Rust.
- Install Node.js and the `katex` npm package if you want SVG math rendering.

### Compiling a single file

Run `dllup-rs` with an input Markdown file and, optionally, a config file.

```
cargo run -- site/index.md dllup.toml
```

### Compiling a directory

```
cargo run -- site dllup.toml
```

Directory builds compile `.md` files.

## Syntax highlighting

Markdown syntax highlighting works out of the box in most editors.

![Syntax highlighting example in vim.](gdTcORo.png)

# FAQ

> Certain features don't work in my browser.

The output of dllup has been W3C validated and tested in the latest versions of Firefox, Google Chrome, and even [Lynx](https://en.wikipedia.org/wiki/Lynx_%28web_browser%29). No attempt shall be made to support other browsers, although you are free to extend dllup as you please.

> Why SVG math instead of MathJax or KaTeX?

Client-side MathJax is slow, causes page content to jump around as math is being rendered, and may freeze the browser on old hardware or slow browsers. Saving math equations as SVG files also allows the SVG files to be cached in case a single equation shows up many times. Some users also prefer to disable Javascript for privacy reasons even when they are using a modern browser that supports SVG images.

> Where can I find an example of a dllup file?

This [documentation itself was written in Markdown-flavoured dllup](index.md).

> The parser crashed.

This can be due to a syntax error in the input. New content should generally be valid Markdown plus the small dllup conventions described above.
