#[macro_use]
extern crate lazy_static;

mod ast;
mod config;
mod html_renderer;
mod image_processor;
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
        eprintln!("Usage: dllup-rs <input.dllu|directory> [config.toml]");
        std::process::exit(1);
    }

    let input_path = Path::new(&args[1]);
    let explicit_config = if let Some(cfg_path) = args.get(2) {
        match config::Config::load(Path::new(cfg_path)) {
            Ok(cfg) => Some(cfg),
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    } else {
        None
    };

    if input_path.is_dir() {
        let files = match collect_dllu_files(input_path) {
            Ok(files) => files,
            Err(e) => {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        };

        if files.is_empty() {
            eprintln!("No .dllu files found in directory {}", input_path.display());
            std::process::exit(1);
        }

        for file in files {
            if let Err(e) = process_file(&file, Some(input_path), explicit_config.as_ref()) {
                eprintln!("{}", e);
                std::process::exit(1);
            }
        }
    } else if let Err(e) = process_file(input_path, input_path.parent(), explicit_config.as_ref()) {
        eprintln!("{}", e);
        std::process::exit(1);
    }
}

fn process_file(
    input_path: &Path,
    site_root: Option<&Path>,
    explicit_config: Option<&config::Config>,
) -> Result<(), String> {
    let config = if let Some(cfg) = explicit_config {
        cfg.clone()
    } else {
        let config_path = config::default_config_path(input_path);
        if config_path.exists() {
            config::Config::load(&config_path)?
        } else {
            config::Config::default()
        }
    };

    let input = fs::read_to_string(input_path)
        .map_err(|e| format!("Failed to read {}: {}", input_path.display(), e))?;

    let t0 = Instant::now();
    let mut parser = Parser::default();
    parser.parse(&input);
    let t_parse = t0.elapsed();

    let t1 = Instant::now();
    let asset_root = input_path
        .parent()
        .map(Path::to_path_buf)
        .unwrap_or_else(|| PathBuf::from("."));
    let mut renderer = html_renderer::HtmlRenderer::with_asset_root(&config, asset_root);
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
    let index_html = build_blog_index(input_path, site_root, &config)?;
    let html =
        html_renderer::wrap_html_document(&config, title, &body, toc_str, &metas, &index_html)
            .map_err(|e| e.to_string())?;
    let t_wrap = t2.elapsed();

    let out_path = input_path.with_extension("html");
    fs::write(&out_path, html)
        .map_err(|e| format!("Failed to write {}: {}", out_path.display(), e))?;

    if config.timings {
        eprintln!(
            "Timings ({}): parse={:?}, render={:?}, wrap={:?}",
            input_path.display(),
            t_parse,
            t_render,
            t_wrap
        );
    }

    Ok(())
}

fn collect_dllu_files(dir: &Path) -> Result<Vec<PathBuf>, String> {
    let mut stack = vec![dir.to_path_buf()];
    let mut files = Vec::new();

    while let Some(path) = stack.pop() {
        let entries = fs::read_dir(&path)
            .map_err(|e| format!("Failed to read directory {}: {}", path.display(), e))?;
        for entry in entries {
            let entry =
                entry.map_err(|e| format!("Failed to read entry in {}: {}", path.display(), e))?;
            let entry_path = entry.path();
            let file_type = entry.file_type().map_err(|e| {
                format!("Failed to read entry type {}: {}", entry_path.display(), e)
            })?;

            if file_type.is_dir() {
                stack.push(entry_path);
            } else if file_type.is_file()
                && entry_path
                    .extension()
                    .and_then(|ext| ext.to_str())
                    .map(|ext| ext.eq_ignore_ascii_case("dllu"))
                    .unwrap_or(false)
            {
                files.push(entry_path);
            }
        }
    }

    files.sort();
    Ok(files)
}

fn build_blog_index(
    input_path: &Path,
    site_root: Option<&Path>,
    config: &config::Config,
) -> Result<String, String> {
    let blog_dir_raw = match config.html.blog_dir.as_deref() {
        Some(dir) if !dir.trim().is_empty() => dir.trim(),
        _ => return Ok(String::new()),
    };

    let blog_dir_clean = blog_dir_raw.trim_matches('/');
    if blog_dir_clean.is_empty() {
        return Ok(String::new());
    }

    let blog_path = blog_dir_clean
        .split('/')
        .filter(|segment| !segment.is_empty())
        .fold(PathBuf::new(), |mut acc, segment| {
            acc.push(segment);
            acc
        });

    let parent_dir = match input_path.parent() {
        Some(dir) => dir,
        None => return Ok(String::new()),
    };

    let matches_blog_dir = if let Some(root) = site_root {
        parent_dir == root.join(&blog_path) || (parent_dir == root && root.ends_with(&blog_path))
    } else {
        parent_dir.ends_with(&blog_path)
    };

    if !matches_blog_dir {
        return Ok(String::new());
    }

    let mut entries = Vec::new();
    let blog_dir_entries = fs::read_dir(parent_dir).map_err(|e| {
        format!(
            "Failed to read blog directory {}: {}",
            parent_dir.display(),
            e
        )
    })?;

    for entry in blog_dir_entries {
        let entry = entry
            .map_err(|e| format!("Failed to read entry in {}: {}", parent_dir.display(), e))?;
        let file_type = entry.file_type().map_err(|e| {
            format!(
                "Failed to read entry type {}: {}",
                entry.path().display(),
                e
            )
        })?;
        if !file_type.is_dir() {
            continue;
        }

        let post_dir = entry.path();
        let source = match find_blog_article_source(&post_dir)? {
            Some(path) => path,
            None => continue,
        };

        let contents = match fs::read_to_string(&source) {
            Ok(c) => c,
            Err(e) => {
                eprintln!("Failed to read blog post {}: {}", source.display(), e);
                continue;
            }
        };

        let mut parser = Parser::default();
        parser.parse(&contents);
        let header = match parser.article.header.as_ref() {
            Some(h) => h,
            None => {
                eprintln!(
                    "Blog post {} missing header; skipping from index",
                    source.display()
                );
                continue;
            }
        };

        let title = header.title.trim();
        if title.is_empty() {
            eprintln!(
                "Blog post {} missing title; skipping from index",
                source.display()
            );
            continue;
        }

        let date = match header.date.as_deref().map(str::trim) {
            Some(d) if !d.is_empty() => d,
            _ => {
                eprintln!(
                    "Blog post {} missing date; skipping from index",
                    source.display()
                );
                continue;
            }
        };

        let slug = match entry.file_name().into_string() {
            Ok(name) => name,
            Err(_) => {
                eprintln!(
                    "Blog directory name {:?} not UTF-8; skipping from index",
                    entry.file_name()
                );
                continue;
            }
        };

        let relative_link = if config.root_url.is_some() {
            build_blog_relative_url(blog_dir_clean, &slug)
        } else {
            slug.clone()
        };
        let href = build_blog_href(config.root_url.as_deref(), &relative_link);
        entries.push(BlogPostIndexEntry {
            title: title.to_string(),
            date_display: date.to_string(),
            date_key: parse_date_key(date),
            href,
        });
    }

    if entries.is_empty() {
        return Ok(String::new());
    }

    entries.sort_by(|a, b| match (a.date_key, b.date_key) {
        (Some(ad), Some(bd)) => bd.cmp(&ad),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.title.cmp(&b.title),
    });

    let mut out = String::from("<nav id=\"blogposts\">");
    for entry in entries {
        out.push_str("<a href=\"");
        out.push_str(&escape_html_attr_simple(&entry.href));
        out.push_str("\"><span class=\"blogdate\">");
        out.push_str(&escape_html_text(&entry.date_display));
        out.push_str("</span><span class=\"blogtitle\">");
        out.push_str(&escape_html_text(&entry.title));
        out.push_str("</span></a>");
    }
    out.push_str("</nav>");

    Ok(out)
}

struct BlogPostIndexEntry {
    title: String,
    date_display: String,
    date_key: Option<(i32, u32, u32)>,
    href: String,
}

fn find_blog_article_source(dir: &Path) -> Result<Option<PathBuf>, String> {
    let index_candidate = dir.join("index.dllu");
    if index_candidate.is_file() {
        return Ok(Some(index_candidate));
    }

    let mut first: Option<PathBuf> = None;
    let entries = fs::read_dir(dir)
        .map_err(|e| format!("Failed to read post directory {}: {}", dir.display(), e))?;
    for entry in entries {
        let entry =
            entry.map_err(|e| format!("Failed to read entry in {}: {}", dir.display(), e))?;
        if entry
            .file_type()
            .map_err(|e| {
                format!(
                    "Failed to read entry type {}: {}",
                    entry.path().display(),
                    e
                )
            })?
            .is_file()
            && entry
                .path()
                .extension()
                .and_then(|ext| ext.to_str())
                .map(|ext| ext.eq_ignore_ascii_case("dllu"))
                .unwrap_or(false)
        {
            first = Some(entry.path());
            break;
        }
    }
    Ok(first)
}

fn build_blog_relative_url(blog_dir: &str, slug: &str) -> String {
    if blog_dir.is_empty() {
        slug.to_string()
    } else if blog_dir.ends_with('/') {
        format!("{}{}", blog_dir, slug)
    } else {
        format!("{}/{}", blog_dir, slug)
    }
}

fn build_blog_href(root_url: Option<&str>, relative: &str) -> String {
    let trimmed_relative = relative.trim_start_matches('/');
    match root_url {
        Some(root) if root == "/" => format!("/{}", trimmed_relative),
        Some(root) => {
            let mut base = root.trim_end_matches('/').to_string();
            if !trimmed_relative.is_empty() {
                base.push('/');
                base.push_str(trimmed_relative);
            }
            base
        }
        None => relative.to_string(),
    }
}

fn parse_date_key(date: &str) -> Option<(i32, u32, u32)> {
    let mut parts = date.splitn(3, '-');
    let year: i32 = parts.next()?.parse().ok()?;
    let month: u32 = parts.next()?.parse().ok()?;
    let day: u32 = parts.next()?.parse().ok()?;
    Some((year, month, day))
}

fn escape_html_attr_simple(input: &str) -> String {
    input
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

fn escape_html_text(input: &str) -> String {
    escape_html_attr_simple(input)
}
