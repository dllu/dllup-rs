use crate::ast::*;
use markdown::{mdast, Constructs, ParseOptions};
use std::collections::HashMap;

#[derive(Debug, Default)]
pub struct Parser {
    pub article: Article,
    image_count: usize,
    display_math_count: usize,
    table_count: usize,
    section_id_counts: HashMap<String, usize>,
}

impl Parser {
    pub fn parse(&mut self, s: &str) {
        self.reset();

        let mdast = match markdown::to_mdast(s, &markdown_parse_options()) {
            Ok(mdast) => mdast,
            Err(err) => {
                eprintln!("markdown parse error: {}", err);
                self.article.body = vec![Block::Paragraph(vec![InlineElement::Text(
                    s.trim().to_string(),
                )])];
                return;
            }
        };

        let children = match mdast {
            mdast::Node::Root(root) => root.children,
            node => vec![node],
        };
        let body_start = if self.article.header.is_none() {
            if let Some(mdast::Node::Yaml(yaml)) = children.first() {
                self.article.header = parse_yaml_header(&yaml.value);
                1
            } else {
                0
            }
        } else {
            0
        };

        self.article.body = self.parse_blocks(&children[body_start..], s);
    }

    fn reset(&mut self) {
        self.article = Article::default();
        self.image_count = 0;
        self.display_math_count = 0;
        self.table_count = 0;
        self.section_id_counts.clear();
    }

    fn parse_blocks(&mut self, children: &[mdast::Node], source: &str) -> Vec<Block> {
        let mut blocks = Vec::new();
        let mut idx = 0usize;

        while idx < children.len() {
            match &children[idx] {
                mdast::Node::Yaml(_) | mdast::Node::Toml(_) | mdast::Node::Definition(_) => {}
                mdast::Node::Html(html) => {
                    blocks.push(Block::Raw(ensure_trailing_newline(html.value.clone())))
                }
                mdast::Node::Code(code) => {
                    blocks.push(Block::CodeBlock {
                        language: code.lang.clone(),
                        code: ensure_trailing_newline(code.value.clone()),
                    });
                }
                mdast::Node::Math(math) => blocks.push(self.display_math_block(&math.value)),
                mdast::Node::Heading(heading) => {
                    blocks
                        .push(self.section_header_block(heading.depth as usize, &heading.children));
                }
                mdast::Node::Blockquote(blockquote) => {
                    blocks.push(Block::BlockQuote(Self::block_children_to_inlines(
                        &blockquote.children,
                    )));
                }
                mdast::Node::List(list) => blocks.push(self.list_block(list)),
                mdast::Node::Table(table) => {
                    let mut consumed_caption = false;
                    let caption = if let Some(caption) =
                        children.get(idx + 1).and_then(table_caption_from_node)
                    {
                        consumed_caption = true;
                        caption
                    } else {
                        Vec::new()
                    };
                    blocks.push(self.table_block(table, caption));
                    if consumed_caption {
                        idx += 1;
                    }
                }
                mdast::Node::Paragraph(paragraph) => {
                    if let Some(button) = Self::big_button_block(&paragraph.children) {
                        blocks.push(button);
                    } else if let Some(images) = Self::paragraph_images(&paragraph.children) {
                        for image in images {
                            blocks.push(self.image_block(image, source));
                        }
                    } else {
                        blocks.push(Block::Paragraph(Self::inlines_from_mdast(
                            &paragraph.children,
                        )));
                    }
                }
                mdast::Node::ThematicBreak(_) => {
                    blocks.push(Block::Raw("<hr>\n".to_string()));
                }
                node => {
                    let text = node.to_string();
                    if !text.trim().is_empty() {
                        blocks.push(Block::Paragraph(Self::parse_inline_markdown(text.trim())));
                    }
                }
            }
            idx += 1;
        }

        blocks
    }

    fn section_header_block(&mut self, level: usize, children: &[mdast::Node]) -> Block {
        let text = plain_text(children).trim().to_string();
        let id = self.generate_id(&text);
        Block::SectionHeader { level, id, text }
    }

    fn image_block(&mut self, image: &mdast::Image, source: &str) -> Block {
        let caption = image_label_source(image, source).unwrap_or_else(|| image.alt.clone());
        let text = Self::parse_inline_markdown(caption.trim());
        let id_number = self.image_count;
        self.image_count += 1;

        Block::ImageFigure {
            url: image.url.trim().to_string(),
            id: None,
            id_number,
            alt: image.alt.trim().to_string(),
            text,
        }
    }

    fn display_math_block(&mut self, content: &str) -> Block {
        let id_number = self.display_math_count;
        self.display_math_count += 1;
        Block::DisplayMath {
            id: None,
            id_number,
            content: content.trim().to_string(),
        }
    }

    fn table_block(&mut self, table: &mdast::Table, caption: Vec<InlineElement>) -> Block {
        let mut rows = table.children.iter().filter_map(|node| {
            if let mdast::Node::TableRow(row) = node {
                Some(table_row_cells(row))
            } else {
                None
            }
        });

        let header = rows.next().unwrap_or_default();
        let body_rows = rows.collect::<Vec<_>>();
        let id_number = self.table_count;
        self.table_count += 1;

        Block::Table {
            id_number,
            header,
            rows: body_rows,
            caption,
        }
    }

    fn list_block(&self, list: &mdast::List) -> Block {
        let mut items = Vec::new();
        collect_list_items(list, 1, &mut items);
        if list.ordered {
            Block::OrderedList(items)
        } else {
            Block::UnorderedList(items)
        }
    }

    fn big_button_block(children: &[mdast::Node]) -> Option<Block> {
        let text = plain_text(children);
        let trimmed = text.trim();
        let rest = trimmed.strip_prefix(":: ")?;
        let (label, url) = rest.rsplit_once(' ')?;
        Some(Block::BigButton {
            text: Self::parse_inline_markdown(label.trim()),
            url: url.trim().to_string(),
        })
    }

    fn paragraph_images(children: &[mdast::Node]) -> Option<Vec<&mdast::Image>> {
        let mut images = Vec::new();
        for child in children {
            match child {
                mdast::Node::Image(image) => images.push(image),
                mdast::Node::Text(text) if text.value.trim().is_empty() => {}
                mdast::Node::Break(_) => {}
                _ => return None,
            }
        }
        if images.is_empty() {
            None
        } else {
            Some(images)
        }
    }

    fn block_children_to_inlines(children: &[mdast::Node]) -> Vec<InlineElement> {
        let mut elements = Vec::new();
        for child in children {
            if !elements.is_empty() {
                elements.push(InlineElement::Text("\n".to_string()));
            }
            match child {
                mdast::Node::Paragraph(paragraph) => {
                    elements.extend(Self::inlines_from_mdast(&paragraph.children));
                }
                mdast::Node::Heading(heading) => {
                    elements.extend(Self::inlines_from_mdast(&heading.children));
                }
                mdast::Node::List(list) => {
                    let mut items = Vec::new();
                    collect_list_items(list, 1, &mut items);
                    for item in items {
                        elements.extend(item.text);
                    }
                }
                node => elements.extend(Self::parse_inline_markdown(&node.to_string())),
            }
        }
        elements
    }

    fn parse_inline_markdown(s: &str) -> Vec<InlineElement> {
        if s.is_empty() {
            return Vec::new();
        }
        match markdown::to_mdast(s, &markdown_parse_options()) {
            Ok(mdast::Node::Root(root)) => {
                if root.children.len() == 1 {
                    if let mdast::Node::Paragraph(paragraph) = &root.children[0] {
                        return Self::inlines_from_mdast(&paragraph.children);
                    }
                }
                vec![InlineElement::Text(s.to_string())]
            }
            _ => vec![InlineElement::Text(s.to_string())],
        }
    }

    fn inlines_from_mdast(children: &[mdast::Node]) -> Vec<InlineElement> {
        let mut elements = Vec::new();
        for child in children {
            elements.extend(Self::inline_from_mdast(child));
        }
        elements
    }

    fn inline_from_mdast(node: &mdast::Node) -> Vec<InlineElement> {
        match node {
            mdast::Node::Text(text) => vec![InlineElement::Text(text.value.clone())],
            mdast::Node::InlineCode(code) => vec![InlineElement::Code(code.value.clone())],
            mdast::Node::InlineMath(math) => vec![InlineElement::InlineMath(math.value.clone())],
            mdast::Node::Emphasis(emphasis) => vec![InlineElement::Emphasis(
                Self::inlines_from_mdast(&emphasis.children),
            )],
            mdast::Node::Strong(strong) => vec![InlineElement::Strong(Self::inlines_from_mdast(
                &strong.children,
            ))],
            mdast::Node::Delete(delete) => Self::inlines_from_mdast(&delete.children),
            mdast::Node::Link(link) => vec![InlineElement::Link {
                text: Self::inlines_from_mdast(&link.children),
                url: link.url.clone(),
            }],
            mdast::Node::LinkReference(reference) => {
                let label = plain_text(&reference.children);
                vec![InlineElement::Text(label)]
            }
            mdast::Node::FootnoteReference(reference) => {
                vec![InlineElement::Text(reference.identifier.clone())]
            }
            mdast::Node::Break(_) => vec![InlineElement::Text("\n".to_string())],
            mdast::Node::Html(html) => vec![InlineElement::RawHtml(html.value.clone())],
            mdast::Node::Image(image) => vec![InlineElement::Text(image.alt.clone())],
            node => {
                let text = node.to_string();
                if text.is_empty() {
                    Vec::new()
                } else {
                    vec![InlineElement::Text(text)]
                }
            }
        }
    }

    fn generate_id(&mut self, text: &str) -> String {
        let base_id = text
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ')
            .collect::<String>()
            .replace(' ', "-");

        let count = self.section_id_counts.entry(base_id.clone()).or_insert(0);
        *count += 1;

        if *count == 1 {
            base_id
        } else {
            format!("{}-{}", base_id, count)
        }
    }
}

fn markdown_parse_options() -> ParseOptions {
    ParseOptions {
        constructs: Constructs {
            frontmatter: true,
            math_flow: true,
            math_text: true,
            ..Constructs::gfm()
        },
        ..ParseOptions::default()
    }
}

fn parse_yaml_header(s: &str) -> Option<ArticleHeader> {
    let mut title = None;
    let mut date = None;
    for line in s.lines() {
        let Some((key, value)) = line.split_once(':') else {
            continue;
        };
        let value = trim_yaml_scalar(value);
        match key.trim() {
            "title" => title = Some(value),
            "date" => date = Some(value),
            _ => {}
        }
    }
    title.map(|title| ArticleHeader { title, date })
}

fn trim_yaml_scalar(value: &str) -> String {
    let trimmed = value.trim();
    if trimmed.len() >= 2 {
        let bytes = trimmed.as_bytes();
        if (bytes[0] == b'"' && bytes[trimmed.len() - 1] == b'"')
            || (bytes[0] == b'\'' && bytes[trimmed.len() - 1] == b'\'')
        {
            return trimmed[1..trimmed.len() - 1].to_string();
        }
    }
    trimmed.to_string()
}

fn table_caption_from_node(node: &mdast::Node) -> Option<Vec<InlineElement>> {
    if let mdast::Node::Paragraph(paragraph) = node {
        strip_table_caption_prefix(&paragraph.children)
            .map(|children| Parser::inlines_from_mdast(&children))
    } else {
        None
    }
}

fn strip_table_caption_prefix(children: &[mdast::Node]) -> Option<Vec<mdast::Node>> {
    let mut result = children.to_vec();
    for idx in 0..result.len() {
        match &mut result[idx] {
            mdast::Node::Text(text) => {
                let trimmed = text.value.trim_start();
                if trimmed.is_empty() {
                    continue;
                }
                let stripped = trimmed.strip_prefix("Table:")?;
                text.value = stripped.trim_start().to_string();
                if text.value.is_empty() {
                    result.remove(idx);
                }
                return Some(result);
            }
            _ => return None,
        }
    }
    None
}

fn table_row_cells(row: &mdast::TableRow) -> Vec<Vec<InlineElement>> {
    row.children
        .iter()
        .filter_map(|node| {
            if let mdast::Node::TableCell(cell) = node {
                Some(Parser::inlines_from_mdast(&cell.children))
            } else {
                None
            }
        })
        .collect()
}

fn collect_list_items(list: &mdast::List, level: usize, items: &mut Vec<crate::ast::ListItem>) {
    for child in &list.children {
        let mdast::Node::ListItem(item) = child else {
            continue;
        };

        let mut text = Vec::new();
        let mut nested_lists = Vec::new();
        for item_child in &item.children {
            match item_child {
                mdast::Node::Paragraph(paragraph) => {
                    if !text.is_empty() {
                        text.push(InlineElement::Text(" ".to_string()));
                    }
                    text.extend(Parser::inlines_from_mdast(&paragraph.children));
                }
                mdast::Node::List(nested) => nested_lists.push(nested),
                node => {
                    let plain = node.to_string();
                    if !plain.trim().is_empty() {
                        if !text.is_empty() {
                            text.push(InlineElement::Text(" ".to_string()));
                        }
                        text.extend(Parser::parse_inline_markdown(plain.trim()));
                    }
                }
            }
        }

        items.push(crate::ast::ListItem { level, text });
        for nested in nested_lists {
            collect_list_items(nested, level + 1, items);
        }
    }
}

fn plain_text(children: &[mdast::Node]) -> String {
    let mut text = String::new();
    for child in children {
        match child {
            mdast::Node::Text(node) => text.push_str(&node.value),
            mdast::Node::InlineCode(node) => text.push_str(&node.value),
            mdast::Node::InlineMath(node) => text.push_str(&node.value),
            mdast::Node::Code(node) => text.push_str(&node.value),
            mdast::Node::Math(node) => text.push_str(&node.value),
            mdast::Node::Html(node) => text.push_str(&node.value),
            mdast::Node::Image(node) => text.push_str(&node.alt),
            mdast::Node::Break(_) => text.push('\n'),
            node => {
                if let Some(grandchildren) = node.children() {
                    text.push_str(&plain_text(grandchildren));
                } else {
                    text.push_str(&node.to_string());
                }
            }
        }
    }
    text
}

fn image_label_source(image: &mdast::Image, source: &str) -> Option<String> {
    let position = image.position.as_ref()?;
    let snippet = source.get(position.start.offset..position.end.offset)?;
    raw_image_label(snippet)
}

fn raw_image_label(snippet: &str) -> Option<String> {
    let rest = snippet.strip_prefix("![")?;
    let mut escaped = false;
    let mut bracket_depth = 0usize;

    for (idx, ch) in rest.char_indices() {
        if escaped {
            escaped = false;
            continue;
        }
        if ch == '\\' {
            escaped = true;
            continue;
        }

        match ch {
            '[' => bracket_depth += 1,
            ']' if bracket_depth == 0 => return Some(rest[..idx].to_string()),
            ']' => bracket_depth = bracket_depth.saturating_sub(1),
            _ => {}
        }
    }

    None
}

fn ensure_trailing_newline(mut s: String) -> String {
    if !s.ends_with('\n') {
        s.push('\n');
    }
    s
}

#[cfg(test)]
mod tests {
    use super::*;

    fn cell_text(cell: &[InlineElement]) -> String {
        cell.iter()
            .map(|inline| match inline {
                InlineElement::Text(s) => s.as_str(),
                _ => "",
            })
            .collect::<Vec<_>>()
            .join("")
    }

    #[test]
    fn parses_markdown_table_with_caption() {
        let input = "---\ntitle: Table Demo\n---\n\n| Colour | Pattern |\n| --- | --- |\n| White | Spots |\n\nTable: Example caption.\n";
        let mut parser = Parser::default();
        parser.parse(input);
        let table = parser
            .article
            .body
            .iter()
            .find_map(|block| {
                if let Block::Table {
                    header,
                    rows,
                    caption,
                    ..
                } = block
                {
                    Some((header, rows, caption))
                } else {
                    None
                }
            })
            .expect("expected table");
        assert_eq!(table.0.len(), 2);
        assert_eq!(cell_text(&table.0[0]), "Colour");
        assert_eq!(cell_text(&table.0[1]), "Pattern");
        assert_eq!(table.1.len(), 1);
        assert_eq!(table.1[0].len(), 2);
        assert_eq!(cell_text(&table.1[0][0]), "White");
        assert_eq!(cell_text(&table.1[0][1]), "Spots");
        assert_eq!(cell_text(table.2), "Example caption.");
    }

    #[test]
    fn parses_markdown_image_as_figure() {
        let input = "---\ntitle: Image Demo\n---\n\n![Caption _text_](photo.jpg)\n";
        let mut parser = Parser::default();
        parser.parse(input);
        let figure = parser
            .article
            .body
            .iter()
            .find_map(|block| {
                if let Block::ImageFigure { alt, text, .. } = block {
                    Some((alt, text))
                } else {
                    None
                }
            })
            .expect("expected image figure");
        assert_eq!(figure.0, "Caption text");
        assert!(figure
            .1
            .iter()
            .any(|el| matches!(el, InlineElement::Emphasis(_))));
    }

    #[test]
    fn parses_inline_html_as_raw_html() {
        let input = "---\ntitle: Reference Demo\n---\n\n- <cite class=\"refname\" id=\"eade\">eade</cite> Eade.\n";
        let mut parser = Parser::default();
        parser.parse(input);
        let list = parser
            .article
            .body
            .iter()
            .find_map(|block| {
                if let Block::UnorderedList(items) = block {
                    Some(items)
                } else {
                    None
                }
            })
            .expect("expected list");
        assert!(list[0]
            .text
            .iter()
            .any(|el| matches!(el, InlineElement::RawHtml(html) if html.contains("id=\"eade\""))));
    }
}
