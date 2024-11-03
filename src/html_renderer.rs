use crate::ast::*;

pub fn render_html(article: &Article) -> String {
    let mut html = String::new();

    // Render the header if present
    if let Some(header) = &article.header {
        html.push_str(&render_header(header));
    }

    // Render each block in the body
    for block in &article.body {
        html.push_str(&render_block(block));
    }

    html
}

fn render_header(header: &ArticleHeader) -> String {
    let mut html = String::new();

    html.push_str("<header>\n");

    // Render the title
    html.push_str(&format!("<h1>{}</h1>\n", escape_html(&header.title)));

    // Render the date if present
    if let Some(date) = &header.date {
        html.push_str(&format!("<p class=\"date\">{}</p>\n", escape_html(date)));
    }

    html.push_str("</header>\n");

    html
}

fn render_block(block: &Block) -> String {
    match block {
        Block::Raw(content) => render_raw_block(content),
        Block::CodeBlock { language, code } => render_code_block(language.as_deref(), code),
        Block::SectionHeader {
            level,
            id,
            id_number,
            text,
        } => render_section_header(*level, id, id_number, text),
        Block::BlockQuote(elements) => render_blockquote(elements),
        Block::ImageFigure { url, text } => render_image_figure(url, text),
        Block::DisplayMath(equation) => render_display_math(equation),
        Block::UnorderedList(items) => render_unordered_list(items),
        Block::OrderedList(items) => render_ordered_list(items),
        Block::Paragraph(elements) => render_paragraph(elements),
    }
}

fn render_raw_block(content: &str) -> String {
    content.to_string()
}

fn render_code_block(language: Option<&str>, code: &str) -> String {
    let lang_class = language.map_or(String::new(), |lang| {
        format!(" class=\"language-{}\"", escape_html(lang))
    });

    format!(
        "<pre><code{}>{}</code></pre>\n",
        lang_class,
        escape_html(code)
    )
}

fn render_section_header(level: usize, id: &str, text: &[InlineElement]) -> String {
    let level = std::cmp::min(level, 6);
    let tag = format!("h{}", level);

    let content = render_inline_elements(text);

    format!("<{} id={}>{}</{}>\n", tag, id, content, tag)
}

fn render_blockquote(elements: &[InlineElement]) -> String {
    let content = render_inline_elements(elements);

    format!("<blockquote>{}</blockquote>\n", content)
}

fn render_image_figure(url: &str, text: &[InlineElement]) -> String {
    let caption = render_inline_elements(text);
    let alt_text = extract_text(text);

    format!(
        "<figure>\n  <img src=\"{}\" alt=\"{}\" />\n  <figcaption>{}</figcaption>\n</figure>\n",
        escape_html(url),
        escape_html(&alt_text),
        caption
    )
}

fn extract_text(elements: &[InlineElement]) -> String {
    elements
        .iter()
        .map(|elem| match elem {
            InlineElement::Text(text) => text.clone(),
            InlineElement::Code(code) => code.clone(),
            InlineElement::InlineMath(math) => math.clone(),
            InlineElement::Link { text, .. } => text.clone(),
            InlineElement::Emphasis(content) => extract_text(content),
            InlineElement::Strong(content) => extract_text(content),
        })
        .collect::<Vec<String>>()
        .join("")
}

fn render_display_math(equation: &str) -> String {
    // Assuming MathJax or KaTeX is used for rendering LaTeX
    format!("$$\n{}\n$$\n", escape_html(equation))
}

fn render_unordered_list(items: &[ListItem]) -> String {
    render_list(items, "<ul>", "</ul>")
}

fn render_ordered_list(items: &[ListItem]) -> String {
    render_list(items, "<ol>", "</ol>")
}

fn render_list(items: &[ListItem], open_tag: &str, close_tag: &str) -> String {
    let mut html = String::new();
    let mut level_stack = vec![String::new()];

    for item in items {
        while level_stack.len() < item.level {
            level_stack.push(String::new());
        }

        while level_stack.len() > item.level {
            let content = level_stack.pop().unwrap();
            level_stack
                .last_mut()
                .unwrap()
                .push_str(&format!("{}{}{}\n", open_tag, content, close_tag));
        }

        let content = render_inline_elements(&item.text);
        level_stack
            .last_mut()
            .unwrap()
            .push_str(&format!("<li>{}</li>\n", content));
    }

    while level_stack.len() > 1 {
        let content = level_stack.pop().unwrap();
        level_stack
            .last_mut()
            .unwrap()
            .push_str(&format!("{}{}{}\n", open_tag, content, close_tag));
    }

    html.push_str(open_tag);
    html.push_str("\n");
    html.push_str(&level_stack.pop().unwrap());
    html.push_str(close_tag);
    html.push_str("\n");

    html
}

fn render_paragraph(elements: &[InlineElement]) -> String {
    let content = render_inline_elements(elements);

    format!("<p>{}</p>\n", content)
}

fn render_inline_elements(elements: &[InlineElement]) -> String {
    elements.iter().map(render_inline_element).collect()
}

fn render_inline_element(element: &InlineElement) -> String {
    match element {
        InlineElement::Text(text) => escape_html(text),
        InlineElement::Code(code) => format!("<code>{}</code>", escape_html(code)),
        InlineElement::InlineMath(math) => format!("${}$", escape_html(math)),
        InlineElement::Link { text, url } => {
            format!("<a href=\"{}\">{}</a>", escape_html(url), escape_html(text))
        }
        InlineElement::Emphasis(content) => {
            let inner = render_inline_elements(content);
            format!("<em>{}</em>", inner)
        }
        InlineElement::Strong(content) => {
            let inner = render_inline_elements(content);
            format!("<strong>{}</strong>", inner)
        }
        InlineElement::Reference(content) => {
            format!("<a class=\"refname\" href=\"#{}\">{}</a>", content, content)
        }
        InlineElement::ReferenceAnchor { content, invisible } => {
            if invisible {
                "".to_string()
            } else {
                format!(
                    "<span class=\"refname\" id=\"#{}\">{}</a>",
                    content, content
                )
            }
        }
    }
}

fn escape_html(s: &str) -> String {
    s.replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#x27;")
        .replace('/', "&#x2F;")
}
