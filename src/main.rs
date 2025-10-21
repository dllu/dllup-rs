#[macro_use]
extern crate lazy_static;

mod ast;
mod config;
mod html_renderer;
mod image_processor;
mod math_engine;
mod parser;

use crate::ast::{Block, InlineElement};
use git2::{DiffOptions, Repository, Status};
use parser::Parser;
use rayon::prelude::*;
use serde::Serialize;
use serde_xml_rs::to_string;
use std::collections::{BTreeMap, HashMap};
use std::convert::TryFrom;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Instant;
use time::{
    format_description::well_known::{Rfc2822, Rfc3339},
    Date, Month, OffsetDateTime, Time, UtcOffset,
};

struct ProcessedPage {
    output_path: PathBuf,
    source_path: PathBuf,
    root_url: Option<String>,
}

#[derive(Clone)]
struct BlogPostIndexEntry {
    title: String,
    date_display: String,
    date_key: Option<(i32, u32, u32)>,
    display_href: String,
    permalink: String,
    summary: Option<String>,
    content_html: String,
}

struct BlogIndex {
    html: String,
    entries: Vec<BlogPostIndexEntry>,
    directory: PathBuf,
    blog_dir: PathBuf,
}

lazy_static! {
    static ref BLOG_POST_CACHE: Mutex<HashMap<PathBuf, BlogPostIndexEntry>> =
        Mutex::new(HashMap::new());
}

#[derive(Serialize)]
#[serde(rename = "urlset")]
struct SitemapUrlSet {
    #[serde(rename = "@xmlns")]
    xmlns: &'static str,
    #[serde(rename = "url")]
    urls: Vec<SitemapUrl>,
}

#[derive(Serialize)]
struct SitemapUrl {
    loc: String,
    lastmod: String,
}

#[derive(Serialize)]
#[serde(rename = "rss")]
struct RssFeed {
    #[serde(rename = "@version")]
    version: &'static str,
    #[serde(rename = "@xmlns:content")]
    content_namespace: &'static str,
    channel: RssChannel,
}

#[derive(Serialize)]
struct RssChannel {
    title: String,
    link: String,
    description: String,
    #[serde(rename = "lastBuildDate", skip_serializing_if = "Option::is_none")]
    last_build_date: Option<String>,
    #[serde(rename = "item")]
    items: Vec<RssItem>,
}

#[derive(Serialize)]
struct RssItem {
    title: String,
    link: String,
    guid: RssGuid,
    #[serde(rename = "pubDate", skip_serializing_if = "Option::is_none")]
    pub_date: Option<String>,
    description: String,
    #[serde(rename = "content:encoded", skip_serializing_if = "Option::is_none")]
    content_encoded: Option<String>,
}

#[derive(Serialize)]
struct RssGuid {
    #[serde(rename = "@isPermaLink")]
    is_perma_link: &'static str,
    #[serde(rename = "#content")]
    value: String,
}

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

        let mut files_by_depth: BTreeMap<usize, Vec<PathBuf>> = BTreeMap::new();
        for file in files {
            let depth = file.components().count();
            files_by_depth.entry(depth).or_default().push(file);
        }

        let mut processed_pages = Vec::new();
        for (_depth, group) in files_by_depth.into_iter().rev() {
            let result: Result<Vec<_>, String> = group
                .into_par_iter()
                .map(|file| process_file(&file, Some(input_path), explicit_config.as_ref()))
                .collect();
            match result {
                Ok(mut pages) => processed_pages.append(&mut pages),
                Err(e) => {
                    eprintln!("{}", e);
                    std::process::exit(1);
                }
            }
        }

        if let Err(e) = generate_sitemap(input_path, &processed_pages) {
            eprintln!("{}", e);
            std::process::exit(1);
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
) -> Result<ProcessedPage, String> {
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
    let blog_index = build_blog_index(input_path, site_root, &config)?;
    let index_html_str = blog_index
        .as_ref()
        .map(|idx| idx.html.as_str())
        .unwrap_or("");
    register_blog_post_if_applicable(input_path, site_root, &config, &parser.article, &body);
    let html =
        html_renderer::wrap_html_document(&config, title, &body, toc_str, &metas, index_html_str)
            .map_err(|e| e.to_string())?;
    let t_wrap = t2.elapsed();

    let out_path = input_path.with_extension("html");
    fs::write(&out_path, html)
        .map_err(|e| format!("Failed to write {}: {}", out_path.display(), e))?;

    if let Some(index_data) = blog_index {
        generate_rss_feed(site_root, &index_data, &config)?;
    }

    let root_url = config.root_url.clone();

    if config.timings {
        eprintln!(
            "Timings ({}): parse={:?}, render={:?}, wrap={:?}",
            input_path.display(),
            t_parse,
            t_render,
            t_wrap
        );
    }

    Ok(ProcessedPage {
        output_path: out_path,
        source_path: input_path.to_path_buf(),
        root_url,
    })
}

fn generate_sitemap(site_root: &Path, pages: &[ProcessedPage]) -> Result<(), String> {
    if pages.is_empty() {
        return Ok(());
    }

    let site_root_canon = site_root.canonicalize().map_err(|e| {
        format!(
            "Failed to canonicalize site root {}: {}",
            site_root.display(),
            e
        )
    })?;

    let repo = Repository::discover(&site_root_canon).ok();
    let repo_workdir = if let Some(repo) = repo.as_ref() {
        if let Some(dir) = repo.workdir() {
            match dir.canonicalize() {
                Ok(path) => Some(path),
                Err(_) => Some(dir.to_path_buf()),
            }
        } else {
            None
        }
    } else {
        None
    };

    let mut global_root_url: Option<String> = None;
    for page in pages {
        if let Some(root_url) = &page.root_url {
            if let Some(existing) = &global_root_url {
                if existing != root_url {
                    return Err(format!(
                        "Conflicting root_url values detected: '{}' vs '{}'",
                        existing, root_url
                    ));
                }
            } else {
                global_root_url = Some(root_url.clone());
            }
        }
    }

    let mut entries = Vec::new();
    for page in pages {
        let output_canon = page.output_path.canonicalize().map_err(|e| {
            format!(
                "Failed to canonicalize output {}: {}",
                page.output_path.display(),
                e
            )
        })?;
        let rel_path = output_canon.strip_prefix(&site_root_canon).map_err(|_| {
            format!(
                "Generated file {} is not inside site root {}",
                output_canon.display(),
                site_root_canon.display()
            )
        })?;
        let relative_url_path = pathbuf_to_url_path(rel_path);

        let page_root_url = page.root_url.as_deref().or(global_root_url.as_deref());

        let loc = if let Some(root_url) = page_root_url {
            build_blog_href(Some(root_url), &relative_url_path)
        } else if relative_url_path.is_empty() {
            "/".to_string()
        } else {
            let trimmed = relative_url_path.trim_start_matches('/');
            format!("/{}", trimmed)
        };

        let source_canon = page.source_path.canonicalize().map_err(|e| {
            format!(
                "Failed to canonicalize source {}: {}",
                page.source_path.display(),
                e
            )
        })?;

        let lastmod = determine_lastmod(repo.as_ref(), repo_workdir.as_deref(), &source_canon)?;

        let lastmod_str = lastmod.format(&Rfc3339).map_err(|e| {
            format!(
                "Failed to format timestamp for {}: {}",
                page.source_path.display(),
                e
            )
        })?;

        entries.push((loc, lastmod_str));
    }

    entries.sort_by(|a, b| a.0.cmp(&b.0));

    let sitemap = SitemapUrlSet {
        xmlns: "http://www.sitemaps.org/schemas/sitemap/0.9",
        urls: entries
            .into_iter()
            .map(|(loc, lastmod)| SitemapUrl { loc, lastmod })
            .collect(),
    };

    let xml = to_string(&sitemap).map_err(|e| format!("Failed to build sitemap XML: {}", e))?;

    let sitemap_path = site_root.join("sitemap.xml");
    fs::write(&sitemap_path, xml)
        .map_err(|e| format!("Failed to write {}: {}", sitemap_path.display(), e))?;

    Ok(())
}

fn determine_lastmod(
    repo: Option<&Repository>,
    repo_workdir: Option<&Path>,
    source_path: &Path,
) -> Result<OffsetDateTime, String> {
    let metadata = fs::metadata(source_path).map_err(|e| {
        format!(
            "Failed to read metadata for {}: {}",
            source_path.display(),
            e
        )
    })?;
    let fs_modified = metadata.modified().map_err(|e| {
        format!(
            "Failed to read modification time for {}: {}",
            source_path.display(),
            e
        )
    })?;
    let fs_time = OffsetDateTime::from(fs_modified);

    let (repo, workdir) = match (repo, repo_workdir) {
        (Some(r), Some(wd)) => (r, wd),
        _ => return Ok(fs_time),
    };

    let relative_path = match source_path.strip_prefix(workdir) {
        Ok(path) => path,
        Err(_) => return Ok(fs_time),
    };

    match repo.status_file(relative_path) {
        Ok(status) => {
            if has_local_changes(status) {
                return Ok(fs_time);
            }
        }
        Err(_) => return Ok(fs_time),
    }

    if let Ok(Some(commit_time)) = git_last_commit_time(repo, relative_path) {
        return Ok(commit_time);
    }

    Ok(fs_time)
}

fn has_local_changes(status: Status) -> bool {
    status.intersects(
        Status::INDEX_NEW
            | Status::INDEX_MODIFIED
            | Status::INDEX_DELETED
            | Status::INDEX_RENAMED
            | Status::INDEX_TYPECHANGE
            | Status::WT_NEW
            | Status::WT_MODIFIED
            | Status::WT_DELETED
            | Status::WT_RENAMED
            | Status::WT_TYPECHANGE
            | Status::CONFLICTED,
    )
}

fn git_last_commit_time(
    repo: &Repository,
    relative_path: &Path,
) -> Result<Option<OffsetDateTime>, git2::Error> {
    let mut revwalk = match repo.revwalk() {
        Ok(walk) => walk,
        Err(_) => return Ok(None),
    };
    if revwalk.push_head().is_err() {
        return Ok(None);
    }
    revwalk.set_sorting(git2::Sort::TIME)?;

    let pathspec = relative_path.to_string_lossy().replace('\\', "/");

    for oid_result in revwalk {
        let oid = match oid_result {
            Ok(id) => id,
            Err(_) => continue,
        };
        let commit = match repo.find_commit(oid) {
            Ok(c) => c,
            Err(_) => continue,
        };
        let tree = match commit.tree() {
            Ok(t) => t,
            Err(_) => continue,
        };

        let parent_tree = if commit.parent_count() > 0 {
            match commit.parent(0) {
                Ok(parent) => parent.tree().ok(),
                Err(_) => None,
            }
        } else {
            None
        };

        let mut diff_opts = DiffOptions::new();
        diff_opts.include_typechange(true);
        diff_opts.pathspec(&pathspec);

        let diff =
            match repo.diff_tree_to_tree(parent_tree.as_ref(), Some(&tree), Some(&mut diff_opts)) {
                Ok(d) => d,
                Err(_) => continue,
            };

        if diff.deltas().len() > 0 {
            let git_time = commit.time();
            if let Ok(dt) = offsetdatetime_from_git_time(git_time) {
                return Ok(Some(dt));
            }
        }
    }

    Ok(None)
}

fn offsetdatetime_from_git_time(
    time: git2::Time,
) -> Result<OffsetDateTime, time::error::ComponentRange> {
    let base = OffsetDateTime::from_unix_timestamp(time.seconds())?;
    let offset = UtcOffset::from_whole_seconds(time.offset_minutes() * 60)?;
    Ok(base.to_offset(offset))
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

    files.sort_by(|a, b| {
        let depth_a = a.components().count();
        let depth_b = b.components().count();
        depth_b.cmp(&depth_a).then_with(|| a.cmp(b))
    });
    Ok(files)
}

fn build_blog_index(
    input_path: &Path,
    site_root: Option<&Path>,
    config: &config::Config,
) -> Result<Option<BlogIndex>, String> {
    let blog_dir_raw = match config.html.blog_dir.as_deref() {
        Some(dir) if !dir.trim().is_empty() => dir.trim(),
        _ => return Ok(None),
    };

    let blog_dir_clean = blog_dir_raw.trim_matches('/');
    if blog_dir_clean.is_empty() {
        return Ok(None);
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
        None => return Ok(None),
    };

    let matches_blog_dir = if let Some(root) = site_root {
        parent_dir == root.join(&blog_path) || (parent_dir == root && root.ends_with(&blog_path))
    } else {
        parent_dir.ends_with(&blog_path)
    };

    if !matches_blog_dir {
        return Ok(None);
    }

    let blog_root = if let Some(root) = site_root {
        root.join(&blog_path)
    } else {
        parent_dir.to_path_buf()
    };

    let mut entries: Vec<BlogPostIndexEntry> = {
        let cache = BLOG_POST_CACHE
            .lock()
            .expect("blog post cache mutex poisoned");
        cache
            .iter()
            .filter_map(|(dir, entry)| {
                if dir
                    .parent()
                    .map(|p| p == blog_root.as_path())
                    .unwrap_or(false)
                {
                    Some(entry.clone())
                } else {
                    None
                }
            })
            .collect()
    };

    if entries.is_empty() {
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

            let summary = first_paragraph_text(&parser.article.body);
            let asset_root = source
                .parent()
                .map(Path::to_path_buf)
                .unwrap_or_else(|| post_dir.clone());
            let mut renderer = html_renderer::HtmlRenderer::with_asset_root(config, asset_root);
            let content_html = renderer.render(&parser.article);
            let relative_path = build_blog_relative_url(blog_dir_clean, &slug);
            let permalink = build_blog_href(config.root_url.as_deref(), &relative_path);
            let display_href = if config.root_url.is_some() {
                permalink.clone()
            } else {
                slug.clone()
            };
            entries.push(BlogPostIndexEntry {
                title: title.to_string(),
                date_display: date.to_string(),
                date_key: parse_date_key(date),
                display_href,
                permalink,
                summary,
                content_html,
            });
        }
    }

    if entries.is_empty() {
        return Ok(None);
    }

    entries.sort_by(|a, b| match (a.date_key, b.date_key) {
        (Some(ad), Some(bd)) => bd.cmp(&ad),
        (Some(_), None) => std::cmp::Ordering::Less,
        (None, Some(_)) => std::cmp::Ordering::Greater,
        (None, None) => a.title.cmp(&b.title),
    });

    let mut out = String::from("<nav id=\"blogposts\">");
    for entry in &entries {
        out.push_str("<a href=\"");
        out.push_str(&escape_html_attr_simple(&entry.display_href));
        out.push_str("\"><span class=\"blogdate\">");
        out.push_str(&escape_html_text(&entry.date_display));
        out.push_str("</span><span class=\"blogtitle\">");
        out.push_str(&escape_html_text(&entry.title));
        out.push_str("</span></a>");
    }
    out.push_str("</nav>");

    Ok(Some(BlogIndex {
        html: out,
        entries,
        directory: parent_dir.to_path_buf(),
        blog_dir: blog_path,
    }))
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
        Some("/") => format!("/{}", trimmed_relative),
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

fn generate_rss_feed(
    _site_root: Option<&Path>,
    blog_index: &BlogIndex,
    config: &config::Config,
) -> Result<(), String> {
    let feed_cfg = &config.feed;
    if !feed_cfg.enabled {
        return Ok(());
    }

    if blog_index.entries.is_empty() {
        return Ok(());
    }

    let blog_relative_root = pathbuf_to_url_path(&blog_index.blog_dir);
    let default_title = if blog_relative_root.is_empty() {
        "Blog".to_string()
    } else {
        blog_relative_root.clone()
    };
    let channel_title = feed_cfg
        .channel_title
        .clone()
        .or_else(|| feed_cfg.title.clone())
        .unwrap_or_else(|| default_title.clone());
    let channel_description = feed_cfg
        .description
        .clone()
        .unwrap_or_else(|| format!("Latest posts from {}", channel_title));
    let default_link = build_blog_href(config.root_url.as_deref(), &blog_relative_root);
    let channel_link = feed_cfg
        .link
        .clone()
        .unwrap_or_else(|| default_link.clone());
    let last_build_date = blog_index
        .entries
        .iter()
        .find_map(|entry| entry.date_key.and_then(date_key_to_rfc2822));
    let max_items = feed_cfg.limit.unwrap_or(blog_index.entries.len());

    let items: Vec<RssItem> = blog_index
        .entries
        .iter()
        .take(max_items)
        .map(|entry| RssItem {
            title: entry.title.clone(),
            link: entry.permalink.clone(),
            guid: RssGuid {
                is_perma_link: "true",
                value: entry.permalink.clone(),
            },
            pub_date: entry.date_key.and_then(date_key_to_rfc2822),
            description: entry.summary.as_deref().unwrap_or(&entry.title).to_string(),
            content_encoded: Some(entry.content_html.clone()),
        })
        .collect();

    let feed = RssFeed {
        version: "2.0",
        content_namespace: "http://purl.org/rss/1.0/modules/content/",
        channel: RssChannel {
            title: channel_title,
            link: channel_link,
            description: channel_description,
            last_build_date,
            items,
        },
    };

    let xml = to_string(&feed).map_err(|e| format!("Failed to build RSS feed XML: {}", e))?;

    let output_path = {
        let candidate = Path::new(&feed_cfg.output_path);
        if candidate.is_absolute() {
            candidate.to_path_buf()
        } else {
            blog_index.directory.join(candidate)
        }
    };

    if let Some(parent) = output_path.parent() {
        fs::create_dir_all(parent).map_err(|e| {
            format!(
                "Failed to create directories for {}: {}",
                output_path.display(),
                e
            )
        })?;
    }

    fs::write(&output_path, xml)
        .map_err(|e| format!("Failed to write {}: {}", output_path.display(), e))?;

    Ok(())
}

fn pathbuf_to_url_path(path: &Path) -> String {
    let mut segments = Vec::new();
    for component in path.iter() {
        let seg = component.to_string_lossy();
        if !seg.is_empty() {
            segments.push(seg.replace('\\', "/"));
        }
    }
    segments.join("/")
}

fn date_key_to_rfc2822(date: (i32, u32, u32)) -> Option<String> {
    let (year, month, day) = date;
    let month = u8::try_from(month).ok()?;
    let month = Month::try_from(month).ok()?;
    let day = u8::try_from(day).ok()?;
    let date = Date::from_calendar_date(year, month, day).ok()?;
    let time = Time::from_hms(0, 0, 0).ok()?;
    let datetime = date.with_time(time).assume_utc();
    datetime.format(&Rfc2822).ok()
}

fn first_paragraph_text(blocks: &[Block]) -> Option<String> {
    for block in blocks {
        if let Block::Paragraph(inlines) = block {
            let text = inline_elements_to_plain_text(inlines);
            let trimmed = text.trim();
            if !trimmed.is_empty() {
                let normalized = collapse_whitespace(trimmed);
                if !normalized.is_empty() {
                    return Some(normalized);
                }
            }
        }
    }
    None
}

fn register_blog_post_if_applicable(
    input_path: &Path,
    site_root: Option<&Path>,
    config: &config::Config,
    article: &ast::Article,
    rendered_body: &str,
) {
    let blog_dir_raw = match config.html.blog_dir.as_deref() {
        Some(dir) if !dir.trim().is_empty() => dir.trim(),
        _ => return,
    };

    let blog_dir_clean = blog_dir_raw.trim_matches('/');
    if blog_dir_clean.is_empty() {
        return;
    }

    let site_root = match site_root {
        Some(root) => root,
        None => return,
    };

    let mut blog_path = PathBuf::new();
    for segment in blog_dir_clean.split('/') {
        if !segment.is_empty() {
            blog_path.push(segment);
        }
    }

    let blog_root = site_root.join(&blog_path);

    let post_dir = match input_path.parent() {
        Some(dir) => dir,
        None => return,
    };

    if post_dir == blog_root {
        return;
    }

    if post_dir
        .parent()
        .map(|parent| parent != blog_root.as_path())
        .unwrap_or(true)
    {
        return;
    }

    let source = match find_blog_article_source(post_dir) {
        Ok(Some(path)) => path,
        _ => return,
    };

    if source != input_path {
        return;
    }

    let header = match article.header.as_ref() {
        Some(h) => h,
        None => return,
    };

    let title = header.title.trim();
    if title.is_empty() {
        return;
    }

    let date = match header.date.as_deref().map(str::trim) {
        Some(d) if !d.is_empty() => d,
        _ => return,
    };

    let slug = match post_dir.file_name().and_then(|s| s.to_str()) {
        Some(s) => s,
        None => return,
    };

    let summary = first_paragraph_text(&article.body);
    let relative_path = build_blog_relative_url(blog_dir_clean, slug);
    let permalink = build_blog_href(config.root_url.as_deref(), &relative_path);
    let display_href = if config.root_url.is_some() {
        permalink.clone()
    } else {
        slug.to_string()
    };

    let entry = BlogPostIndexEntry {
        title: title.to_string(),
        date_display: date.to_string(),
        date_key: parse_date_key(date),
        display_href,
        permalink,
        summary,
        content_html: rendered_body.to_string(),
    };

    if let Ok(mut cache) = BLOG_POST_CACHE.lock() {
        cache.insert(post_dir.to_path_buf(), entry);
    }
}

fn inline_elements_to_plain_text(inlines: &[InlineElement]) -> String {
    let mut out = String::new();
    for inline in inlines {
        match inline {
            InlineElement::Text(t) => out.push_str(t),
            InlineElement::Code(c) | InlineElement::InlineMath(c) => out.push_str(c),
            InlineElement::Link { text, .. } => out.push_str(&inline_elements_to_plain_text(text)),
            InlineElement::Emphasis(inner) | InlineElement::Strong(inner) => {
                out.push_str(&inline_elements_to_plain_text(inner))
            }
            InlineElement::Reference(s) => out.push_str(s),
            InlineElement::ReferenceAnchor { content, .. } => out.push_str(content),
        }
    }
    out
}

fn collapse_whitespace(input: &str) -> String {
    let mut result = String::new();
    let mut last_was_space = false;
    for ch in input.chars() {
        if ch.is_whitespace() {
            if !last_was_space && !result.is_empty() {
                result.push(' ');
            }
            last_was_space = true;
        } else {
            result.push(ch);
            last_was_space = false;
        }
    }
    if last_was_space {
        result.pop();
    }
    result
}
