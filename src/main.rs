#[macro_use]
extern crate lazy_static;

mod ast;
mod config;
mod html_renderer;
mod math_engine;
mod parser;

use parser::Parser;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::time::Instant;

fn main() {
    let args: Vec<String> = env::args().collect();
    if args.len() < 2 || args.len() > 3 {
        eprintln!("Usage: dllup-rs <input.dllu> [config.toml]");
        std::process::exit(1);
    }

    let input_path = Path::new(&args[1]);
    let config_path = args
        .get(2)
        .map(PathBuf::from)
        .unwrap_or_else(|| config::default_config_path(input_path));

    let config_should_load = args.len() == 3 || config_path.exists();
    let config = if config_should_load {
        match config::Config::load(&config_path) {
            Ok(cfg) => cfg,
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    } else {
        config::Config::default()
    };

    let input = match fs::read_to_string(input_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!("Failed to read {}: {}", input_path.display(), e);
            std::process::exit(1);
        }
    };

    let t0 = Instant::now();
    let mut parser = Parser::default();
    parser.parse(&input);
    let t_parse = t0.elapsed();

    let t1 = Instant::now();
    let mut renderer = html_renderer::HtmlRenderer::new(&config);
    let body = renderer.render(&parser.article);
    let t_render = t1.elapsed();
    let title = parser
        .article
        .header
        .as_ref()
        .map(|h| h.title.as_str())
        .unwrap_or("Document");
    let t2 = Instant::now();
    let toc_html = renderer.table_of_contents_html();
    let toc_str = toc_html.as_deref().unwrap_or("");
    let metas = renderer.meta_tags(title);
    let html = match html_renderer::wrap_html_document(&config, title, &body, toc_str, &metas) {
        Ok(html) => html,
        Err(e) => {
            eprintln!("{}", e);
            std::process::exit(1);
        }
    };
    let t_wrap = t2.elapsed();

    let out_path = input_path.with_extension("html");
    if let Err(e) = fs::write(&out_path, html) {
        eprintln!("Failed to write {}: {}", out_path.display(), e);
        std::process::exit(1);
    }

    if config.timings {
        eprintln!(
            "Timings: parse={:?}, render={:?}, wrap={:?}",
            t_parse, t_render, t_wrap
        );
    }
}
