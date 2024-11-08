#[macro_use]
extern crate lazy_static;

mod ast;
// mod html_renderer;
mod parser;

use parser::Parser;

fn main() {
    let input = r#"
My Article Title
2024-11-01
===

# Introduction

Welcome to my blog post. This is a _simple_ example of **strong** text and `code`.
Another sentence in the same paragraph with inline math: $y = mx + b$.
Here is a [link](https://purplepuppy.com) and a [link with **formatted** _text_](https://example.com).

???
This is a raw block. It will be copied as is.
???

$ E = mc^2

pic https://example.com/image.png: An example image

* Item 1
** Subitem 1.1
* Item 2

1. First
2. Second

> This is a blockquote.

~~~ 
lang rust
fn main() {
    println!("Hello, world!");
}
~~~
"#;

    let mut parser = Parser::default();
    parser.parse(input);

    println!("{:#?}", parser.article);
}
