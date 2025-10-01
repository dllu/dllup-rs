use crate::ast::*;
use crate::math_engine::{ExternalCmdEngine, MathEngine};
use inkjet::formatter::ThemedHtml;
use inkjet::theme::vendored::ONEDARKER;
use inkjet::theme::Theme;
use inkjet::{Highlighter, Language};
use regex::Regex;
use std::env;
use std::fs;

pub struct HtmlRenderer {
    engine: Option<Box<dyn MathEngine>>, // external command or none
    memo_math: std::collections::HashMap<(bool, String), String>,
}

impl HtmlRenderer {
    pub fn new() -> Self {
        Self {
            engine: Self::make_engine_from_env(),
            memo_math: std::collections::HashMap::new(),
        }
    }

    fn make_engine_from_env() -> Option<Box<dyn MathEngine>> {
        // Prefer V8 engine if built-in feature is enabled
        // Prefer persistent katex node process if available
        if env::var("DLLUP_PERSISTENT_KATEX").ok().as_deref() == Some("1") {
            match crate::math_engine::PersistentKatexEngine::spawn() {
                Ok(engine) => return Some(Box::new(engine)),
                Err(e) => eprintln!("Failed to spawn persistent KaTeX: {}. Falling back.", e),
            }
        }
        if let Ok(s) = env::var("DLLUP_KATEX_CMD").or_else(|_| env::var("DLLUP_MATH_CMD")) {
            let parts = shell_words::split(&s).unwrap_or_else(|_| vec![s]);
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
        let mut html = String::new();

        if let Some(header) = &article.header {
            html.push_str(&self.render_header(header));
        }

        for block in &article.body {
            html.push_str(&self.render_block(block));
        }

        html
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
            Block::SectionHeader {
                level, id, text, ..
            } => self.render_section_header(*level, id, text),
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
            Block::Paragraph(elements) => self.render_paragraph(elements),
            Block::Table {
                id_number,
                header,
                rows,
                caption,
            } => self.render_table(*id_number, header, rows, caption),
            Block::BigButton { text, url } => {
                let inner = self.render_inlines(text);
                format!(
                    "<p><a href=\"{}\" class=\"bigbutton\">{}</a></p>\n",
                    escape_html(url),
                    inner
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

    fn render_section_header(&self, level: usize, id: &str, text: &str) -> String {
        let level = std::cmp::min(level, 6);
        let tag = format!("h{}", level);
        format!(
            "<{} id=\"{}\"><span>{}</span></{}>\n",
            tag,
            escape_html(id),
            escape_html(text),
            tag
        )
    }

    fn render_image_figure(
        &mut self,
        url: &str,
        id: Option<&str>,
        id_number: usize,
        alt: &str,
        text: &[InlineElement],
    ) -> String {
        let fig_id_num = id_number + 1;
        let fig_id_attr = id
            .map(|s| escape_html(s))
            .unwrap_or_else(|| format!("fig{}", fig_id_num));

        let caption_html = self.render_inlines(text);
        let alt_text = alt.to_string();

        format!(
            "<figure id=\"{}\"><a href=\"{}\"><img src=\"{}\" alt=\"{}\"/></a><figcaption><a href=\"#{}\" class=\"fignum\">FIGURE {}</a> {}</figcaption></figure>\n",
            fig_id_attr,
            escape_html(url),
            escape_html(url),
            escape_html(&alt_text),
            fig_id_attr,
            fig_id_num,
            caption_html
        )
    }

    fn render_display_math(&mut self, id: Option<&str>, id_number: usize, content: &str) -> String {
        let eqnum = id_number + 1;
        let eq_id_attr = id
            .map(|s| escape_html(s))
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
                format!("<a href=\"{}\">{}</a>", escape_html(url), inner)
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
}

// removed SVG metric extraction: KaTeX HTML is inlined directly

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
    if !punct.contains(&next_first) && !prev_last.is_whitespace() {
        if prev_last == '>' && next_first.is_alphanumeric() {
            // avoid double spaces if already present
            if prev.ends_with(' ') || next.starts_with(' ') {
                return false;
            }
            return true;
        }
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
        header: &Vec<Vec<InlineElement>>,
        rows: &Vec<Vec<Vec<InlineElement>>>,
        caption: &Vec<InlineElement>,
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

pub fn wrap_html_document(title: &str, body: &str) -> String {
    // Try to load static/template.html and static/styles.css
    let template_path = "static/template.html";
    let css_href = "static/styles.css";
    match fs::read_to_string(template_path) {
        Ok(tpl) => {
            let page = tpl
                .replace("{{title}}", &html_escape_attr(title))
                .replace("{{css}}", css_href)
                .replace("{{body}}", body);
            page
        }
        Err(_) => {
            // Fallback inline template if static files are missing
            let fallback = format!(
                "<!DOCTYPE html><html lang=\"en\"><head><meta charset=\"utf-8\"><meta name=\"viewport\" content=\"width=device-width, initial-scale=1\">\
                <title>{}</title>\
                <link rel=\"stylesheet\" href=\"https://cdn.jsdelivr.net/npm/katex@0.16.22/dist/katex.min.css\" integrity=\"sha384-5TcZemv2l/9On385z///+d7MSYlvIEw9FuZTIdZ14vJLqWphw7e7ZPuOiCHJcFCP\" crossorigin=\"anonymous\">\
                <style>body{{max-width:820px;margin:2rem auto;padding:0 1rem;line-height:1.65;font-family:-apple-system,BlinkMacSystemFont,'Segoe UI',Roboto,Ubuntu,'Helvetica Neue',Arial,sans-serif}}</style></head><body>{}</body></html>",
                html_escape_attr(title), body
            );
            fallback
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fallback_inline_math() {
        let mut r = HtmlRenderer { engine: None };
        let html = r.render_paragraph(&[
            InlineElement::Text("A ".into()),
            InlineElement::InlineMath("x+y".into()),
            InlineElement::Text(" B".into()),
        ]);
        assert!(html.contains("<span class=\"math-inline\">x+y</span>"));
    }

    #[test]
    fn render_figure_alt_and_caption() {
        let mut r = HtmlRenderer { engine: None };
        let caption = vec![
            InlineElement::Text("An ".into()),
            InlineElement::Emphasis(vec![InlineElement::Text("example".into())]),
        ];
        let html = r.render_image_figure("/img.png", None, 0, "An example", &caption);
        assert!(html.contains("FIGURE 1"));
        assert!(html.contains("alt=\"An example\""));
    }
}
