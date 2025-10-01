#[macro_use]
extern crate lazy_static;

mod ast;
mod html_renderer;
mod math_engine;
mod parser;

use parser::Parser;
use std::env;
use std::fs;
use std::path::Path;
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() != 2 {
        eprintln!("Usage: dllup-rs <input.dllu>");
        std::process::exit(1);
    }

    let input_path = &args[1];
    let input = match fs::read_to_string(input_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to read {}: {}", input_path, e);
            std::process::exit(1);
        }
    };

    let t0 = Instant::now();
    let mut parser = Parser::default();
    parser.parse(&input);
    let t_parse = t0.elapsed();

    let t1 = Instant::now();
    let mut renderer = html_renderer::HtmlRenderer::new();
    let body = renderer.render(&parser.article);
    let t_render = t1.elapsed();
    let title = parser
        .article
        .header
        .as_ref()
        .map(|h| h.title.as_str())
        .unwrap_or("Document");
    let t2 = Instant::now();
    let html = html_renderer::wrap_html_document(title, &body);
    let t_wrap = t2.elapsed();

    let out_path = Path::new(input_path).with_extension("html");
    if let Err(e) = fs::write(&out_path, html) {
        eprintln!("Failed to write {}: {}", out_path.display(), e);
        std::process::exit(1);
    }

    if env::var("DLLUP_TIMINGS").ok().as_deref() == Some("1") {
        eprintln!(
            "Timings: parse={:?}, render={:?}, wrap={:?}",
            t_parse, t_render, t_wrap
        );
    }
}
