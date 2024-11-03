use crate::ast::*;
use lazy_static;
use regex::Regex;
use std::collections::HashMap;
use std::str::Lines;

#[derive(Debug, Default)]
pub struct Parser {
    pub article: Article,
    section_headers: Vec<usize>,
    image_figures: Vec<usize>,
    display_equations: Vec<usize>,

    section_id_counts: HashMap<String, usize>,
}

impl Parser {
    pub fn parse(&mut self, s: &str) {
        let parts: Vec<&str> = s.split("\n===\n").collect();
        if parts.len() > 1 {
            self.article.header = Some(self.parse_header(parts[0]));
            self.article.body = self.parse_body(parts[1]);
        } else {
            self.article.header = None;
            self.article.body = self.parse_body(parts[0]);
        }
    }

    fn parse_header(&self, s: &str) -> ArticleHeader {
        let mut lines = s.lines().filter(|line| !line.trim().is_empty());
        let title = lines.next().unwrap_or_default().to_string();
        let date = lines.next().map(|line| line.to_string());

        ArticleHeader { title, date }
    }

    fn parse_body(&mut self, s: &str) -> Vec<Block> {
        let mut blocks = Vec::new();
        let mut lines = s.lines().peekable();

        while let Some(_) = lines.peek() {
            if let Some(block) = self.parse_block(&mut lines) {
                let ind = blocks.len();
                match &block {
                    Block::ImageFigure { .. } => {
                        self.image_figures.push(ind);
                    }
                    Block::DisplayMath { .. } => {
                        self.display_equations.push(ind);
                    }
                    Block::SectionHeader { .. } => {
                        self.section_headers.push(ind);
                    }
                    _ => {}
                }
                blocks.push(block)
            }
        }
        blocks
    }

    fn parse_block(&mut self, lines: &mut std::iter::Peekable<Lines>) -> Option<Block> {
        while let Some(&line) = lines.peek() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                lines.next();
                continue;
            }

            if trimmed == "???" {
                return Some(Self::parse_raw_block(lines));
            } else if trimmed == "~~~" {
                return Some(Self::parse_code_block(lines));
            } else if trimmed.starts_with('#') {
                return Some(self.parse_section_header(lines));
            } else if trimmed.starts_with("> ") {
                return Some(self.parse_blockquote(lines));
            } else if trimmed.starts_with("pic ") {
                return Some(self.parse_image_figure(lines));
            } else if trimmed.starts_with("$ ") {
                return Some(self.parse_display_math(lines));
            } else if Self::is_unordered_list_item(trimmed) {
                return Some(Self::parse_unordered_list(lines));
            } else if trimmed.starts_with("1. ") {
                return Some(Self::parse_ordered_list(lines));
            } else {
                return Some(Self::parse_paragraph(lines));
            }
        }

        None
    }

    fn is_unordered_list_item(s: &str) -> bool {
        // if you have more than 6 levels of list nesting then you're insane
        s.starts_with("* ")
            || s.starts_with("** ")
            || s.starts_with("*** ")
            || s.starts_with("**** ")
            || s.starts_with("***** ")
            || s.starts_with("****** ")
    }

    fn is_ordered_list_item(s: &str) -> bool {
        lazy_static! {
            static ref ORDERED_LIST_REGEX: Regex = Regex::new(r"^\d+\. ").unwrap();
        }
        ORDERED_LIST_REGEX.is_match(s)
    }

    fn parse_raw_block(lines: &mut std::iter::Peekable<Lines>) -> Block {
        // Consume the starting "???"
        lines.next();

        let mut content = String::new();

        while let Some(&line) = lines.peek() {
            let trimmed = line.trim();
            if trimmed == "???" {
                // Consume the ending "???"
                lines.next();
                break;
            } else {
                content.push_str(line);
                content.push('\n');
                lines.next();
            }
        }

        Block::Raw(content)
    }

    fn parse_code_block(lines: &mut std::iter::Peekable<Lines>) -> Block {
        // Consume the starting "~~~"
        lines.next();

        let mut language = None;
        let mut code = String::new();

        if let Some(&line) = lines.peek() {
            let trimmed = line.trim();
            if trimmed.starts_with("lang ") {
                language = Some(trimmed[5..].to_string());
                lines.next();
            }
        }

        while let Some(&line) = lines.peek() {
            let trimmed = line.trim();
            if trimmed == "~~~" {
                // Consume the ending "~~~"
                lines.next();
                break;
            } else {
                code.push_str(line);
                code.push('\n');
                lines.next();
            }
        }

        Block::CodeBlock { language, code }
    }

    fn generate_id(&mut self, text: &str) -> String {
        // Generate a URL-friendly ID from the text
        let base_id = text
            .to_lowercase()
            .chars()
            .filter(|c| c.is_alphanumeric() || *c == ' ')
            .collect::<String>()
            .replace(' ', "-");

        // Ensure the ID is unique
        let count = self.section_id_counts.entry(base_id.clone()).or_insert(0);
        *count += 1;

        if *count == 1 {
            base_id
        } else {
            format!("{}-{}", base_id, count)
        }
    }

    fn parse_section_header(&mut self, lines: &mut std::iter::Peekable<Lines>) -> Block {
        if let Some(line) = lines.next() {
            let trimmed = line.trim();
            let level = trimmed.chars().take_while(|&c| c == '#').count();
            let text = trimmed[level..].trim();
            let id = self.generate_id(text);
            let id_number = self.section_headers.len();

            Block::SectionHeader {
                level,
                id,
                id_number,
                text: text.to_string(),
            }
        } else {
            // Should not reach here
            Block::Paragraph(vec![])
        }
    }

    fn parse_blockquote(&self, lines: &mut std::iter::Peekable<Lines>) -> Block {
        let mut content = String::new();

        while let Some(&line) = lines.peek() {
            let trimmed = line.trim();
            if trimmed.starts_with("> ") {
                content.push_str(&trimmed[2..]);
                content.push('\n');
                lines.next();
            } else {
                break;
            }
        }

        Block::BlockQuote(Self::parse_inline_elements(&content))
    }

    fn parse_image_figure(&self, lines: &mut std::iter::Peekable<Lines>) -> Block {
        if let Some(line) = lines.next() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix("pic ") {
                if let Some((url, text)) = rest.split_once(':') {
                    let mut text = Self::parse_inline_elements(text.trim());
                    let mut id = None;
                    for element in &mut text {
                        match element {
                            InlineElement::ReferenceAnchor {
                                content,
                                ref mut invisible,
                            } => {
                                *invisible = true;
                                id = Some(content);
                            }
                            _ => {}
                        }
                    }
                    return Block::ImageFigure {
                        url: url.trim().to_string(),
                        id: id.cloned(),
                        id_number: self.image_figures.len(),
                        text,
                    };
                }
            }
        }

        Block::Paragraph(vec![])
    }

    fn parse_display_math(&self, lines: &mut std::iter::Peekable<Lines>) -> Block {
        if let Some(line) = lines.next() {
            let trimmed = line.trim();
            if let Some(math) = trimmed.strip_prefix("$ ") {
                return Block::DisplayMath {
                    id: None,
                    id_number: self.display_equations.len(),
                    content: math.to_string(),
                };
            }
        }

        Block::Paragraph(vec![])
    }

    fn parse_unordered_list(lines: &mut std::iter::Peekable<Lines>) -> Block {
        let mut items = Vec::new();

        while let Some(&line) = lines.peek() {
            let trimmed = line.trim();

            if Self::is_unordered_list_item(trimmed) {
                let level = trimmed.chars().take_while(|&c| c == '*').count();
                let content = trimmed[level..].trim();
                items.push(ListItem {
                    level,
                    text: Self::parse_inline_elements(content),
                });
                lines.next();
            } else if trimmed.is_empty() {
                lines.next(); // Skip empty line
                break;
            } else {
                break;
            }
        }

        Block::UnorderedList(items)
    }

    fn parse_ordered_list(lines: &mut std::iter::Peekable<Lines>) -> Block {
        let mut items = Vec::new();

        while let Some(&line) = lines.peek() {
            let trimmed = line.trim();

            if Self::is_ordered_list_item(trimmed) {
                let parts: Vec<&str> = trimmed.splitn(2, '.').collect();
                if let Ok(number) = parts[0].parse::<usize>() {
                    let content = parts[1].trim();
                    items.push(ListItem {
                        level: number,
                        text: Self::parse_inline_elements(content),
                    });
                    lines.next();
                } else {
                    break;
                }
            } else if trimmed.is_empty() {
                lines.next(); // Skip empty line
                break;
            } else {
                break;
            }
        }

        Block::OrderedList(items)
    }

    fn parse_paragraph(lines: &mut std::iter::Peekable<Lines>) -> Block {
        let mut content = String::new();

        while let Some(&line) = lines.peek() {
            let trimmed = line.trim();

            if trimmed.is_empty() {
                lines.next(); // Skip empty line
                break;
            } else {
                content.push_str(line);
                content.push('\n');
                lines.next();
            }
        }

        Block::Paragraph(Self::parse_inline_elements(&content.trim_end()))
    }

    fn parse_inline_elements(s: &str) -> Vec<InlineElement> {
        let mut elements = Vec::new();
        let mut chars = s.chars().peekable();
        let mut buffer = String::new();

        while let Some(&c) = chars.peek() {
            match c {
                '`' => {
                    if !buffer.is_empty() {
                        elements.push(InlineElement::Text(buffer.clone()));
                        buffer.clear();
                    }
                    chars.next(); // Skip '`'
                    let code = chars.by_ref().take_while(|&ch| ch != '`').collect();
                    chars.next(); // Skip closing '`'
                    elements.push(InlineElement::Code(code));
                }
                '$' => {
                    if !buffer.is_empty() {
                        elements.push(InlineElement::Text(buffer.clone()));
                        buffer.clear();
                    }
                    chars.next(); // Skip '$'
                    let math = chars.by_ref().take_while(|&ch| ch != '$').collect();
                    chars.next(); // Skip closing '$'
                    elements.push(InlineElement::InlineMath(math));
                }
                '[' => {
                    if !buffer.is_empty() {
                        elements.push(InlineElement::Text(buffer.clone()));
                        buffer.clear();
                    }
                    chars.next(); // Skip '['
                    let link_text: String = chars.by_ref().take_while(|&ch| ch != ']').collect();
                    chars.next(); // Skip ']'
                    if chars.next() == Some('(') {
                        let url: String = chars.by_ref().take_while(|&ch| ch != ')').collect();
                        chars.next(); // Skip ')'
                        elements.push(InlineElement::Link {
                            text: Self::parse_inline_elements(&link_text),
                            url,
                        });
                    }
                }
                '_' => {
                    if !buffer.is_empty() {
                        elements.push(InlineElement::Text(buffer.clone()));
                        buffer.clear();
                    }
                    chars.next(); // Skip '_'
                    let mut emph_text = String::new();
                    while let Some(&ch) = chars.peek() {
                        if ch == '_' {
                            chars.next(); // Skip closing '_'
                            break;
                        } else {
                            emph_text.push(ch);
                            chars.next();
                        }
                    }
                    elements.push(InlineElement::Emphasis(Self::parse_inline_elements(
                        &emph_text,
                    )));
                }
                '*' => {
                    chars.next(); // Skip first '*'
                    if let Some(&next_char) = chars.peek() {
                        if next_char == '*' {
                            chars.next(); // Skip second '*'
                            if !buffer.is_empty() {
                                elements.push(InlineElement::Text(buffer.clone()));
                                buffer.clear();
                            }
                            let mut strong_text = String::new();
                            while let Some(&ch) = chars.peek() {
                                if ch == '*' {
                                    chars.next(); // Skip '*'
                                    if let Some(&next_ch) = chars.peek() {
                                        if next_ch == '*' {
                                            chars.next(); // Skip second '*'
                                            break;
                                        } else {
                                            strong_text.push('*');
                                        }
                                    } else {
                                        strong_text.push('*');
                                    }
                                } else {
                                    strong_text.push(ch);
                                    chars.next();
                                }
                            }
                            elements.push(InlineElement::Strong(Self::parse_inline_elements(
                                &strong_text,
                            )));
                        } else {
                            buffer.push('*');
                        }
                    } else {
                        buffer.push('*');
                    }
                }
                _ => {
                    buffer.push(c);
                    chars.next();
                }
            }
        }

        if !buffer.is_empty() {
            elements.push(InlineElement::Text(buffer));
        }

        elements
    }
}
