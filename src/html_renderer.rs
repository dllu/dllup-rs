use crate::ast::*;
use crate::config;
use crate::math_engine::{ExternalCmdEngine, MathEngine};
use inkjet::formatter::ThemedHtml;
use inkjet::theme::vendored::ONEDARKER;
use inkjet::theme::Theme;
use inkjet::{Highlighter, Language};
use regex::Regex;
use std::borrow::Cow;
use std::fs;

pub struct HtmlRenderer {
    engine: Option<Box<dyn MathEngine>>, // external command or none
    memo_math: std::collections::HashMap<(bool, String), String>,
    config: config::Config,
    toc: Vec<TocEntry>,
    section_counters: Vec<usize>,
    meta_description: Option<String>,
    meta_image: Option<String>,
}

#[derive(Debug, Clone)]
struct TocEntry {
    level: usize,
    title: String,
    numbering_label: String,
    anchor_id: String,
}

impl HtmlRenderer {
    pub fn new(config: &config::Config) -> Self {
        Self {
            engine: Self::make_engine_from_config(config),
            memo_math: std::collections::HashMap::new(),
            config: config.clone(),
            toc: Vec::new(),
            section_counters: Vec::new(),
            meta_description: None,
            meta_image: None,
        }
    }

    fn make_engine_from_config(config: &config::Config) -> Option<Box<dyn MathEngine>> {
        // Prefer V8 engine if built-in feature is enabled
        // Prefer persistent katex node process if available
        if config.math.prefer_persistent {
            match crate::math_engine::PersistentKatexEngine::spawn() {
                Ok(engine) => return Some(Box::new(engine)),
                Err(e) => eprintln!("Failed to spawn persistent KaTeX: {}. Falling back.", e),
            }
        }
        if let Some(command) = &config.math.command {
            let parts = shell_words::split(command).unwrap_or_else(|_| vec![command.clone()]);
            return Some(Box::new(ExternalCmdEngine { cmd: parts }));
        }
        // Default to persistent node engine; if that fails, fallback to npx katex
        if let Ok(engine) = crate::math_engine::PersistentKatexEngine::spawn() {
            return Some(Box::new(engine));
        }
        Some(Box::new(ExternalCmdEngine {
            cmd: vec!["npx".into(), "katex".into()],
        }))
    }

    pub fn render(&mut self, article: &Article) -> String {
        self.toc.clear();
        self.section_counters.clear();
        self.meta_description = None;
        self.meta_image = None;
        let mut html = String::new();

        if let Some(header) = &article.header {
            html.push_str(&self.render_header(header));
        }

        for block in &article.body {
            html.push_str(&self.render_block(block));
        }

        html
    }

    pub fn table_of_contents_html(&self) -> Option<String> {
        if self.toc.is_empty() {
            return None;
        }

        let mut html = String::from("<div class=\"toc\">");
        let mut current_level = 0usize;

        for entry in &self.toc {
            let level = entry.level;
            if level > current_level {
                for _ in current_level..level {
                    html.push_str("<ol>");
                }
            } else {
                html.push_str("</li>");
                for _ in level..current_level {
                    html.push_str("</ol></li>");
                }
            }
            html.push_str("<li>");
            html.push_str(&toc_link(entry));
            current_level = level;
        }

        for _ in 0..current_level {
            html.push_str("</li></ol>");
        }

        html.push_str("</div>");
        Some(html)
    }

    pub fn meta_tags(&self, title: &str) -> String {
        let mut tags = Vec::new();
        if let Some(image) = &self.meta_image {
            tags.push(format!(
                "<meta property=\"og:image\" content=\"{}\" />",
                html_escape_attr(image)
            ));
        }

        if let Some(description) = &self.meta_description {
            let escaped = html_escape_attr(description);
            tags.push(format!(
                "<meta property=\"og:description\" content=\"{}\" />",
                escaped
            ));
            tags.push(format!(
                "<meta name=\"description\" content=\"{}\" />",
                escaped
            ));
        }

        tags.push(format!(
            "<meta property=\"og:title\" content=\"{}\" />",
            html_escape_attr(title)
        ));

        let twitter_card = if self.meta_image.is_some() {
            "summary_large_image"
        } else {
            "summary"
        };
        tags.push(format!(
            "<meta name=\"twitter:card\" content=\"{}\">",
            twitter_card
        ));

        tags.push("<meta name=\"robots\" content=\"max-image-preview:large\">".to_string());

        if tags.is_empty() {
            return String::new();
        }

        let mut result = String::new();
        for (idx, tag) in tags.iter().enumerate() {
            if idx > 0 {
                result.push('\n');
                result.push_str("  ");
            }
            result.push_str(tag);
        }
        result
    }

    fn render_header(&self, header: &ArticleHeader) -> String {
        let mut html = String::new();
        html.push_str("<header>\n");
        html.push_str(&format!(
            "<h1 id=\"top\">{}</h1>\n",
            escape_html(&header.title)
        ));
        if let Some(date) = &header.date {
            html.push_str(&format!("<p class=\"date\">{}</p>\n", escape_html(date)));
        }
        html.push_str("</header>\n");
        html
    }

    fn render_block(&mut self, block: &Block) -> String {
        match block {
            Block::Raw(content) => content.to_string(),
            Block::CodeBlock { language, code } => {
                self.render_code_block(language.as_deref(), code)
            }
            Block::SectionHeader { level, id, text } => {
                self.render_section_header(*level, id, text)
            }
            Block::BlockQuote(elements) => {
                let content = self.render_inlines(elements);
                format!("<blockquote>{}</blockquote>\n", content)
            }
            Block::ImageFigure {
                url,
                id,
                id_number,
                alt,
                text,
            } => self.render_image_figure(url, id.as_deref(), *id_number, alt, text),
            Block::DisplayMath {
                id,
                id_number,
                content,
            } => self.render_display_math(id.as_deref(), *id_number, content),
            Block::UnorderedList(items) => self.render_unordered_list(items),
            Block::OrderedList(items) => self.render_ordered_list(items),
            Block::Paragraph(elements) => {
                self.capture_description(elements);
                self.render_paragraph(elements)
            }
            Block::Table {
                id_number,
                header,
                rows,
                caption,
            } => self.render_table(*id_number, header, rows, caption),
            Block::BigButton { text, url } => {
                let inner = self.render_inlines(text);
                let href = self.escape_url(url);
                format!(
                    "<p><a href=\"{}\" class=\"bigbutton\">{}</a></p>\n",
                    href, inner
                )
            }
        }
    }

    fn render_code_block(&self, language: Option<&str>, code: &str) -> String {
        // Try inkjet syntax highlighting; fall back to plain code block
        match highlight_with_inkjet(language, code) {
            Some(html) => html,
            None => {
                let lang_class = language
                    .map(|l| format!(" class=\"language-{}\"", escape_html(l)))
                    .unwrap_or_default();
                format!(
                    "<pre><code{}>{}</code></pre>\n",
                    lang_class,
                    escape_html(code)
                )
            }
        }
    }

    fn render_section_header(&mut self, level: usize, id: &str, text: &str) -> String {
        let level = std::cmp::min(level, 6);
        let tag = format!("h{}", level);
        let anchor_id = self.register_section(level, text);
        format!(
            "<{} id=\"{}\"><span id=\"{}\">{}</span></{}>\n",
            tag,
            escape_html(id),
            escape_html(&anchor_id),
            escape_html(text),
            tag
        )
    }

    fn register_section(&mut self, level: usize, text: &str) -> String {
        let level = level.clamp(1, 6);
        if self.section_counters.len() < level {
            self.section_counters.resize(level, 0);
        }
        self.section_counters[level - 1] += 1;
        for idx in level..self.section_counters.len() {
            self.section_counters[idx] = 0;
        }
        let numbering_label = self.section_counters[..level]
            .iter()
            .map(|n| n.to_string())
            .collect::<Vec<_>>()
            .join(".");
        let anchor_id = format!("s{}", numbering_label);
        self.toc.push(TocEntry {
            level,
            title: text.to_string(),
            numbering_label: numbering_label.clone(),
            anchor_id: anchor_id.clone(),
        });
        anchor_id
    }

    fn capture_description(&mut self, elements: &[InlineElement]) {
        if self.meta_description.is_some() {
            return;
        }
        let text = extract_text(elements).trim().to_string();
        if !text.is_empty() {
            self.meta_description = Some(text);
        }
    }

    fn capture_image(&mut self, url: &str) {
        if self.meta_image.is_some() {
            return;
        }
        let trimmed = url.trim();
        if trimmed.is_empty() {
            return;
        }
        let resolved = self.url_with_root(trimmed);
        self.meta_image = Some(resolved.into_owned());
    }

    fn render_image_figure(
        &mut self,
        url: &str,
        id: Option<&str>,
        id_number: usize,
        alt: &str,
        text: &[InlineElement],
    ) -> String {
        self.capture_image(url);
        let fig_id_num = id_number + 1;
        let fig_id_attr = id
            .map(escape_html)
            .unwrap_or_else(|| format!("fig{}", fig_id_num));

        let caption_html = self.render_inlines(text);
        let alt_text = alt.to_string();

        let href = self.escape_url(url);
        let src = self.escape_url(url);
        format!(
            "<figure id=\"{}\"><a href=\"{}\"><img src=\"{}\" alt=\"{}\"/></a><figcaption><a href=\"#{}\" class=\"fignum\">FIGURE {}</a> {}</figcaption></figure>\n",
            fig_id_attr,
            href,
            src,
            escape_html(&alt_text),
            fig_id_attr,
            fig_id_num,
            caption_html
        )
    }

    fn render_display_math(&mut self, id: Option<&str>, id_number: usize, content: &str) -> String {
        let eqnum = id_number + 1;
        let eq_id_attr = id
            .map(escape_html)
            .unwrap_or_else(|| format!("eq{}", eqnum));

        let html = self.render_math_html(content, false);
        format!(
            "<div class=\"math\" id=\"{}\"><a href=\"#{}\" class=\"eqnum\">{}</a> {}</div>\n",
            eq_id_attr, eq_id_attr, eqnum, html
        )
    }

    fn render_unordered_list(&mut self, items: &[ListItem]) -> String {
        // Build nested lists properly: each deeper level nests inside the previous <li>
        if items.is_empty() {
            return String::new();
        }
        let mut out = String::new();
        let mut prev_level = 0usize;
        let mut first_flags: Vec<bool> = Vec::new(); // per depth, true if next item is first at that depth

        for item in items {
            let lvl = item.level;
            while prev_level < lvl {
                out.push_str("<ul>");
                prev_level += 1;
                first_flags.push(true);
            }
            while prev_level > lvl {
                out.push_str("</li></ul>");
                prev_level -= 1;
                first_flags.pop();
            }
            if let Some(is_first) = first_flags.last() {
                if !*is_first {
                    out.push_str("</li>");
                }
            }
            let content = self.render_inlines(&item.text);
            out.push_str("<li>");
            out.push_str(&content);
            if let Some(last) = first_flags.last_mut() {
                *last = false;
            }
        }
        // close remaining
        if !first_flags.is_empty() {
            out.push_str("</li>");
        }
        while prev_level > 0 {
            out.push_str("</ul>");
            prev_level -= 1;
        }
        out.push('\n');
        out
    }

    fn render_ordered_list(&mut self, items: &[ListItem]) -> String {
        // The parser stores the number in `level`, but we render as a simple <ol>
        let mut out = String::new();
        out.push_str("<ol>");
        for item in items {
            out.push_str("<li>");
            out.push_str(&self.render_inlines(&item.text));
            out.push_str("</li>");
        }
        out.push_str("</ol>\n");
        out
    }

    fn render_paragraph(&mut self, elements: &[InlineElement]) -> String {
        let content = self.render_inlines(elements);
        format!("<p>{}</p>\n", content)
    }

    fn render_inlines(&mut self, elements: &[InlineElement]) -> String {
        let mut out = String::new();
        for el in elements {
            let frag = self.render_inline(el);
            if needs_space_between(&out, &frag) {
                out.push(' ');
            }
            out.push_str(&frag);
        }
        out
    }

    fn render_inline(&mut self, element: &InlineElement) -> String {
        match element {
            InlineElement::Text(text) => typographer(text),
            InlineElement::Code(code) => format!("<code>{}</code>", escape_html(code)),
            InlineElement::InlineMath(math) => self.render_math_html(math, true),
            InlineElement::Link { text, url } => {
                let inner = self.render_inlines(text);
                let href = self.escape_url(url);
                format!("<a href=\"{}\">{}</a>", href, inner)
            }
            InlineElement::Emphasis(content) => {
                let inner = self.render_inlines(content);
                format!("<em>{}</em>", inner)
            }
            InlineElement::Strong(content) => {
                let inner = self.render_inlines(content);
                format!("<strong>{}</strong>", inner)
            }
            InlineElement::Reference(content) => {
                let esc = escape_html(content);
                format!("<a class=\"refname\" href=\"#{}\">{}</a>", esc, esc)
            }
            InlineElement::ReferenceAnchor { content, invisible } => {
                if *invisible {
                    String::new()
                } else {
                    let esc = escape_html(content);
                    format!("<span class=\"refname\" id=\"{}\">{}</span>", esc, esc)
                }
            }
        }
    }

    fn render_math_html(&mut self, latex: &str, inline: bool) -> String {
        // For display mode, wrap in an aligned environment unless already present
        let wrapped = if inline {
            latex.to_string()
        } else {
            let has_align = latex.contains("\\begin{align") || latex.contains("\\begin{aligned}");
            if has_align {
                latex.to_string()
            } else {
                format!("\\begin{{aligned}}{}\\end{{aligned}}", latex)
            }
        };

        if let Some(cached) = self.memo_math.get(&(inline, wrapped.clone())) {
            return cached.clone();
        }
        if let Some(engine) = self.engine.as_deref_mut() {
            match engine.tex_to_html(&wrapped, inline) {
                Ok(s) if !s.trim().is_empty() => return s,
                Ok(_) => {}
                Err(e) => eprintln!("math render error: {}", e),
            }
        }
        // Fallback: just show the raw TeX in a code span/div
        if inline {
            format!("<span class=\"math-inline\">{}</span>", escape_html(latex))
        } else {
            let s = format!("<div class=\"math-display\">{}</div>", escape_html(latex));
            self.memo_math.insert((inline, wrapped), s.clone());
            s
        }
    }

    fn escape_url(&self, url: &str) -> String {
        let resolved = self.url_with_root(url);
        escape_html(&resolved)
    }

    fn url_with_root<'a>(&self, url: &'a str) -> Cow<'a, str> {
        match self.config.root_url.as_deref() {
            Some(root) if url.starts_with('/') && !url.starts_with("//") => {
                if root == "/" {
                    Cow::Borrowed(url)
                } else {
                    Cow::Owned(format!("{}{}", root, url))
                }
            }
            _ => Cow::Borrowed(url),
        }
    }
}

// removed SVG metric extraction: KaTeX HTML is inlined directly

fn toc_link(entry: &TocEntry) -> String {
    let href = format!("#{}", entry.anchor_id);
    format!(
        "<a href=\"{}\"><span class=\"tocnum\">{}</span> <span>{}</span></a>",
        html_escape_attr(&href),
        html_escape_attr(&entry.numbering_label),
        escape_html(&entry.title)
    )
}

fn extract_text(elements: &[InlineElement]) -> String {
    let mut out = String::new();
    for el in elements {
        match el {
            InlineElement::Text(t) => out.push_str(t),
            InlineElement::Code(c) => out.push_str(c),
            InlineElement::InlineMath(m) => out.push_str(m),
            InlineElement::Link { text, .. } => out.push_str(&extract_text(text)),
            InlineElement::Emphasis(inner) | InlineElement::Strong(inner) => {
                out.push_str(&extract_text(inner))
            }
            InlineElement::Reference(s) => out.push_str(s),
            InlineElement::ReferenceAnchor { content, .. } => out.push_str(content),
        }
    }
    out
}

fn escape_html(s: &str) -> String {
    html_escape_attr(s)
}

fn html_escape_attr(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
}

fn needs_space_between(prev: &str, next: &str) -> bool {
    if prev.is_empty() || next.is_empty() {
        return false;
    }
    let prev_last = prev
        .chars()
        .rev()
        .find(|c| !c.is_whitespace())
        .unwrap_or(' ');
    let next_first = next.chars().find(|c| !c.is_whitespace()).unwrap_or(' ');
    // Insert space when HTML tag boundary meets a word character
    let punct = [',', '.', ';', ':', '?', '!', ')', ']', '}', '"', '\'', '/'];
    if !punct.contains(&next_first)
        && !prev_last.is_whitespace()
        && prev_last == '>'
        && next_first.is_alphanumeric()
    {
        // avoid double spaces if already present
        if prev.ends_with(' ') || next.starts_with(' ') {
            return false;
        }
        return true;
    }
    false
}

fn typographer(input: &str) -> String {
    let mut s = input.to_string();
    // Dashes, ellipsis first
    s = s.replace("---", "—");
    s = s.replace("--", "–");
    s = s.replace("...", "…");

    // Opening double quotes at start or after whitespace
    let re_dq1 = Regex::new(r#"(^|\s)\""#).unwrap();
    s = re_dq1.replace_all(&s, "$1“").to_string();
    // Opening single quotes at start or after non-word char
    let re_sq1 = Regex::new(r"(^|[^A-Za-z0-9_])'([A-Za-z0-9_])").unwrap();
    s = re_sq1.replace_all(&s, "$1‘$2").to_string();

    // Remaining quotes to closing quotes
    s = s.replace('"', "”");
    s = s.replace("'", "’");

    // Remove single backslashes used as escapes (not double)
    s = unescape_backslashes(&s);

    html_escape_attr(&s)
}

fn unescape_backslashes(s: &str) -> String {
    let mut out = String::with_capacity(s.len());
    let mut prev_was_backslash = false;
    for ch in s.chars() {
        if ch == '\\' {
            if prev_was_backslash {
                // keep one backslash
                out.push('\\');
                prev_was_backslash = false;
            } else {
                // mark and skip for now
                prev_was_backslash = true;
            }
        } else {
            if prev_was_backslash {
                // previous was a single backslash: drop it
                prev_was_backslash = false;
            }
            out.push(ch);
        }
    }
    // trailing single backslash gets dropped
    out
}

impl HtmlRenderer {
    fn render_table(
        &mut self,
        id_number: usize,
        header: &[Vec<InlineElement>],
        rows: &[Vec<Vec<InlineElement>>],
        caption: &[InlineElement],
    ) -> String {
        let table_id = format!("table{}", id_number + 1);
        let mut out = String::new();
        out.push_str(&format!("<figure id=\"{}\"><table>", table_id));
        out.push_str("<tr>");
        for cell in header {
            out.push_str("<th>");
            out.push_str(&self.render_inlines(cell));
            out.push_str("</th>");
        }
        out.push_str("</tr>");
        for row in rows {
            out.push_str("<tr>");
            for cell in row {
                out.push_str("<td>");
                out.push_str(&self.render_inlines(cell));
                out.push_str("</td>");
            }
            out.push_str("</tr>");
        }
        out.push_str("</table>");
        let caption_html = self.render_inlines(caption);
        out.push_str(&format!(
            "<figcaption><a href=\"#{}\" class=\"fignum\">Table {}</a> {}</figcaption>",
            table_id,
            id_number + 1,
            caption_html
        ));
        out.push_str("</figure>\n");
        out
    }
}

fn highlight_with_inkjet(language: Option<&str>, code: &str) -> Option<String> {
    let mut highlighter = Highlighter::new();
    let theme = Theme::from_helix(ONEDARKER).ok()?;
    let formatter = ThemedHtml::new(theme);
    let lang = language.and_then(Language::from_token).unwrap_or_else(|| {
        Language::from_token("plaintext").unwrap_or(Language::from_token("none").unwrap())
    });
    match highlighter.highlight_to_string(lang, &formatter, code) {
        Ok(s) => Some(s),
        Err(_) => None,
    }
}

pub fn wrap_html_document(
    config: &config::Config,
    title: &str,
    body: &str,
    table_of_contents: &str,
    metas: &str,
) -> Result<String, String> {
    let template_path = &config.html.template_path;
    let template = fs::read_to_string(template_path)
        .map_err(|e| format!("failed to read HTML template {}: {}", template_path, e))?;

    let css_href = html_escape_attr(&css_href_with_root(config));

    Ok(template
        .replace("{{title}}", &html_escape_attr(title))
        .replace("{{css}}", &css_href)
        .replace("{{tableofcontents}}", table_of_contents)
        .replace("{{metas}}", metas)
        .replace("{{body}}", body))
}

fn css_href_with_root(config: &config::Config) -> String {
    let raw = config.html.css_href.trim();
    if raw.is_empty() {
        return String::new();
    }

    match config.root_url.as_deref() {
        Some(root) if raw.starts_with('/') && !raw.starts_with("//") => {
            if root == "/" {
                raw.to_string()
            } else {
                format!("{}{}", root, raw)
            }
        }
        Some(root) if is_relative_href(raw) => {
            let trimmed = raw.trim_start_matches('/');
            if root == "/" {
                format!("/{}", trimmed)
            } else {
                format!("{}/{}", root, trimmed)
            }
        }
        _ => raw.to_string(),
    }
}

fn is_relative_href(href: &str) -> bool {
    !href.is_empty()
        && !href.starts_with('/')
        && !href.starts_with('#')
        && !href.starts_with("//")
        && !href.contains(':')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_inline_math() {
        let mut r = HtmlRenderer {
            engine: None,
            memo_math: std::collections::HashMap::new(),
            config: crate::config::Config::default(),
            toc: Vec::new(),
            section_counters: Vec::new(),
            meta_description: None,
            meta_image: None,
        };
        let html = r.render_paragraph(&[
            InlineElement::Text("A ".into()),
            InlineElement::InlineMath("x+y".into()),
            InlineElement::Text(" B".into()),
        ]);
        assert!(html.contains("<span class=\"math-inline\">x+y</span>"));
    }

    #[test]
    fn render_figure_alt_and_caption() {
        let mut r = HtmlRenderer {
            engine: None,
            memo_math: std::collections::HashMap::new(),
            config: crate::config::Config::default(),
            toc: Vec::new(),
            section_counters: Vec::new(),
            meta_description: None,
            meta_image: None,
        };
        let caption = vec![
            InlineElement::Text("An ".into()),
            InlineElement::Emphasis(vec![InlineElement::Text("example".into())]),
        ];
        let html = r.render_image_figure("/img.png", None, 0, "An example", &caption);
        assert!(html.contains("FIGURE 1"));
        assert!(html.contains("alt=\"An example\""));
    }

    #[test]
    fn root_url_prefixes_internal_links() {
        let mut cfg = crate::config::Config::default();
        cfg.root_url = Some("https://example.com".into());
        let mut r = HtmlRenderer {
            engine: None,
            memo_math: std::collections::HashMap::new(),
            config: cfg,
            toc: Vec::new(),
            section_counters: Vec::new(),
            meta_description: None,
            meta_image: None,
        };
        let html = r.render_inlines(&[InlineElement::Link {
            text: vec![InlineElement::Text("link".into())],
            url: "/foo.html".into(),
        }]);
        assert!(html.contains("href=\"https://example.com/foo.html\""));
    }

    #[test]
    fn css_href_respects_root_for_relative_paths() {
        let mut cfg = crate::config::Config::default();
        cfg.root_url = Some("https://example.com/blog".into());
        cfg.html.css_href = "static/styles.css".into();
        let href = super::css_href_with_root(&cfg);
        assert_eq!(href, "https://example.com/blog/static/styles.css");
    }

    #[test]
    fn css_href_respects_root_for_root_relative_paths() {
        let mut cfg = crate::config::Config::default();
        cfg.root_url = Some("https://example.com".into());
        cfg.html.css_href = "/assets/styles.css".into();
        let href = super::css_href_with_root(&cfg);
        assert_eq!(href, "https://example.com/assets/styles.css");
    }

    #[test]
    fn table_of_contents_for_math_example_matches_expected() {
        use crate::parser::Parser;
        use std::fs;

        let source = fs::read_to_string("example/math.dllu").expect("math example fixture");
        let mut parser = Parser::default();
        parser.parse(&source);

        let mut renderer = HtmlRenderer::new(&crate::config::Config::default());
        renderer.render(&parser.article);
        let toc = renderer
            .table_of_contents_html()
            .expect("expected table of contents");

        let expected = r##"<div class="toc"><ol><li><a href="#s1"><span class="tocnum">1</span> <span>Transformation parameterisation</span></a><ol><li><a href="#s1.1"><span class="tocnum">1.1</span> <span>Transforming a point</span></a></li><li><a href="#s1.2"><span class="tocnum">1.2</span> <span>The Lie algebra</span></a></li><li><a href="#s1.3"><span class="tocnum">1.3</span> <span>The exponential map</span></a></li><li><a href="#s1.4"><span class="tocnum">1.4</span> <span>Notation summary</span></a></li></ol></li><li><a href="#s2"><span class="tocnum">2</span> <span>Derivatives</span></a></li><li><a href="#s3"><span class="tocnum">3</span> <span>Optimisation</span></a><ol><li><a href="#s3.1"><span class="tocnum">3.1</span> <span>Optimisation under uncertainty</span></a></li><li><a href="#s3.2"><span class="tocnum">3.2</span> <span>Robust loss functions</span></a></li></ol></li><li><a href="#s4"><span class="tocnum">4</span> <span>Trajectory representation</span></a></li><li><a href="#s5"><span class="tocnum">5</span> <span>Parameterisation of the perturbation</span></a></li><li><a href="#s6"><span class="tocnum">6</span> <span>Constraints</span></a><ol><li><a href="#s6.1"><span class="tocnum">6.1</span> <span>Position constraint</span></a><ol><li><a href="#s6.1.1"><span class="tocnum">6.1.1</span> <span>Residual</span></a></li><li><a href="#s6.1.2"><span class="tocnum">6.1.2</span> <span>Left Jacobian</span></a></li><li><a href="#s6.1.3"><span class="tocnum">6.1.3</span> <span>Right Jacobian</span></a></li></ol></li><li><a href="#s6.2"><span class="tocnum">6.2</span> <span>Loop closure constraint</span></a><ol><li><a href="#s6.2.1"><span class="tocnum">6.2.1</span> <span>Residual</span></a></li><li><a href="#s6.2.2"><span class="tocnum">6.2.2</span> <span>Left Jacobians</span></a></li><li><a href="#s6.2.3"><span class="tocnum">6.2.3</span> <span>Right Jacobians</span></a></li></ol></li><li><a href="#s6.3"><span class="tocnum">6.3</span> <span>Gravity constraint</span></a><ol><li><a href="#s6.3.1"><span class="tocnum">6.3.1</span> <span>Residual</span></a></li><li><a href="#s6.3.2"><span class="tocnum">6.3.2</span> <span>Left Jacobian</span></a></li><li><a href="#s6.3.3"><span class="tocnum">6.3.3</span> <span>Right Jacobian</span></a></li></ol></li><li><a href="#s6.4"><span class="tocnum">6.4</span> <span>Point constraint</span></a><ol><li><a href="#s6.4.1"><span class="tocnum">6.4.1</span> <span>Residual</span></a></li><li><a href="#s6.4.2"><span class="tocnum">6.4.2</span> <span>Left Jacobian</span></a></li><li><a href="#s6.4.3"><span class="tocnum">6.4.3</span> <span>Right Jacobian</span></a></li></ol></li><li><a href="#s6.5"><span class="tocnum">6.5</span> <span>Velocity constraint</span></a><ol><li><a href="#s6.5.1"><span class="tocnum">6.5.1</span> <span>Residual</span></a></li><li><a href="#s6.5.2"><span class="tocnum">6.5.2</span> <span>Left Jacobian</span></a></li></ol></li><li><a href="#s6.6"><span class="tocnum">6.6</span> <span>Regularisation constraint</span></a><ol><li><a href="#s6.6.1"><span class="tocnum">6.6.1</span> <span>Residual</span></a></li><li><a href="#s6.6.2"><span class="tocnum">6.6.2</span> <span>Jacobian</span></a></li></ol></li></ol></li><li><a href="#s7"><span class="tocnum">7</span> <span>References</span></a></li></ol></div>"##;

        assert_eq!(toc, expected);
    }

    #[test]
    fn metas_for_chickenrice_example() {
        use crate::parser::Parser;
        use std::fs;

        let source =
            fs::read_to_string("example/chickenrice.dllu").expect("chicken rice example fixture");
        let mut parser = Parser::default();
        parser.parse(&source);

        let mut renderer = HtmlRenderer {
            engine: None,
            memo_math: std::collections::HashMap::new(),
            config: crate::config::Config::default(),
            toc: Vec::new(),
            section_counters: Vec::new(),
            meta_description: None,
            meta_image: None,
        };

        renderer.render(&parser.article);
        let title = parser
            .article
            .header
            .as_ref()
            .map(|h| h.title.as_str())
            .unwrap_or("Document");
        let metas = renderer.meta_tags(title);

        let expected = concat!(
            "<meta property=\"og:image\" content=\"https://pics.dllu.net/file/dllu-pics/2020-05-29-18-17-47_DSCF5250_adec3569ef61415da569075c8d18157b9f6790ee_1200.jpg\" />\n  ",
            "<meta property=\"og:description\" content=\"Hainanese Chicken Rice (海南鸡饭), more commonly referred to as just &quot;chicken rice&quot; is a Singaporean dish consisting of poached chicken, rice cooked in chicken broth, soy sauce, ginger, and garlic.\n",
            "Over time, the dish has evolved significantly, following several waves of Chinese immigration to Singapore.\n",
            "The history of the dish is roughly as follows:\" />\n  ",
            "<meta name=\"description\" content=\"Hainanese Chicken Rice (海南鸡饭), more commonly referred to as just &quot;chicken rice&quot; is a Singaporean dish consisting of poached chicken, rice cooked in chicken broth, soy sauce, ginger, and garlic.\n",
            "Over time, the dish has evolved significantly, following several waves of Chinese immigration to Singapore.\n",
            "The history of the dish is roughly as follows:\" />\n  ",
            "<meta property=\"og:title\" content=\"Chicken rice in San Francisco\" />\n  ",
            "<meta name=\"twitter:card\" content=\"summary_large_image\">\n  ",
            "<meta name=\"robots\" content=\"max-image-preview:large\">"
        );

        assert_eq!(metas, expected);
    }
}
