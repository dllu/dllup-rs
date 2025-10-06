use crate::ast::*;
use regex::Regex;
use std::collections::HashMap;
use std::str::Lines;

#[derive(Debug, Default)]
pub struct Parser {
    pub article: Article,
    section_headers: Vec<usize>,
    image_figures: Vec<usize>,
    display_equations: Vec<usize>,
    tables: Vec<usize>,

    section_id_counts: HashMap<String, usize>,
}

impl Parser {
    pub fn parse(&mut self, s: &str) {
        let parts: Vec<&str> = s.splitn(2, "\n===\n").collect();
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

        while lines.peek().is_some() {
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
                    Block::Table { .. } => {
                        self.tables.push(ind);
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
            } else if trimmed == "~~~~" {
                return Some(Self::parse_code_block_nohighlight(lines));
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
            } else if trimmed.starts_with("| ") {
                return Some(self.parse_table(lines));
            } else if trimmed.starts_with(":: ") {
                return Some(self.parse_big_button(lines));
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

    fn parse_code_block_nohighlight(lines: &mut std::iter::Peekable<Lines>) -> Block {
        // Consume the starting "~~~~"
        lines.next();

        let mut code = String::new();

        while let Some(&line) = lines.peek() {
            let trimmed = line.trim();
            if trimmed == "~~~~" {
                // Consume the ending "~~~~"
                lines.next();
                break;
            } else {
                code.push_str(line);
                code.push('\n');
                lines.next();
            }
        }

        Block::CodeBlock {
            language: None,
            code,
        }
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
                if let Some((left, caption)) = rest.split_once(" : ") {
                    let left = left.trim();
                    // left contains: URL and then ALT text
                    let mut parts = left.split_whitespace();
                    let url = parts.next().unwrap_or("").to_string();
                    let alt = parts.collect::<Vec<_>>().join(" ");

                    let mut text = Self::parse_inline_elements(caption.trim());
                    let mut id = None;
                    for element in &mut text {
                        if let InlineElement::ReferenceAnchor {
                            content,
                            ref mut invisible,
                        } = element
                        {
                            *invisible = true;
                            id = Some(content);
                        }
                    }
                    return Block::ImageFigure {
                        url: url.trim().to_string(),
                        id: id.cloned(),
                        id_number: self.image_figures.len(),
                        alt: alt.trim().to_string(),
                        text,
                    };
                }
            }
        }

        Block::Paragraph(vec![])
    }

    fn parse_display_math(&self, lines: &mut std::iter::Peekable<Lines>) -> Block {
        let mut content = String::new();
        if let Some(line) = lines.next() {
            let trimmed = line.trim();
            if let Some(math) = trimmed.strip_prefix("$ ") {
                content.push_str(math);
                // collect subsequent non-empty lines as part of the display math block
                while let Some(&next_line) = lines.peek() {
                    let t = next_line.trim();
                    if t.is_empty() {
                        lines.next();
                        break;
                    }
                    // Stop if a new block starts (conservative: allow most content inside math)
                    if t == "???"
                        || t == "~~~"
                        || t == "~~~~"
                        || t.starts_with("#")
                        || t.starts_with("> ")
                        || t.starts_with("pic ")
                        || t.starts_with("| ")
                        || t.starts_with(":: ")
                        || Self::is_unordered_list_item(t)
                        || Self::is_ordered_list_item(t)
                    {
                        break;
                    }
                    content.push('\n');
                    content.push_str(next_line);
                    lines.next();
                }
                return Block::DisplayMath {
                    id: None,
                    id_number: self.display_equations.len(),
                    content,
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
                lines.next(); // end of list block
                break;
            } else {
                // Continuation line for previous list item (multiline <li>)
                if !items.is_empty() {
                    let mut extra = Self::parse_inline_elements(trimmed);
                    if let Some(last) = items.last_mut() {
                        last.text.push(InlineElement::Text(" ".into()));
                        last.text.append(&mut extra);
                    }
                    lines.next();
                } else {
                    break;
                }
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
                let content = parts[1].trim();
                // level is always 1 for flat ordered lists (nesting not supported here)
                items.push(ListItem {
                    level: 1,
                    text: Self::parse_inline_elements(content),
                });
                lines.next();
            } else if trimmed.is_empty() {
                lines.next(); // Skip empty line
                break;
            } else {
                // Continuation for previous list item
                if !items.is_empty() {
                    let mut extra = Self::parse_inline_elements(trimmed);
                    if let Some(last) = items.last_mut() {
                        last.text.push(InlineElement::Text(" ".into()));
                        last.text.append(&mut extra);
                    }
                    lines.next();
                } else {
                    break;
                }
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
                // Stop paragraph if hitting the start of a new block
                if trimmed == "???"
                    || trimmed == "~~~~"
                    || trimmed == "~~~"
                    || trimmed.starts_with('#')
                    || trimmed.starts_with("> ")
                    || trimmed.starts_with("pic ")
                    || trimmed.starts_with("$ ")
                    || trimmed.starts_with("| ")
                    || trimmed.starts_with(":: ")
                    || Self::is_unordered_list_item(trimmed)
                    || Self::is_ordered_list_item(trimmed)
                {
                    break;
                }
                content.push_str(line);
                content.push('\n');
                lines.next();
            }
        }

        Block::Paragraph(Self::parse_inline_elements(content.trim_end()))
    }

    fn parse_inline_elements(s: &str) -> Vec<InlineElement> {
        let mut elements = Vec::new();
        let chars: Vec<char> = s.chars().collect();
        let mut i = 0usize;
        let mut buffer = String::new();
        while i < chars.len() {
            let c = chars[i];
            // escape: treat next char literally
            if c == '\\' {
                if i + 1 < chars.len() {
                    buffer.push(chars[i + 1]);
                    i += 2;
                } else {
                    i += 1;
                }
                continue;
            }
            // code span
            if c == '`' {
                if !buffer.is_empty() {
                    elements.push(InlineElement::Text(buffer.clone()));
                    buffer.clear();
                }
                i += 1; // skip opening
                let mut code = String::new();
                while i < chars.len() {
                    if chars[i] == '\\' && i + 1 < chars.len() && chars[i + 1] == '`' {
                        code.push('`');
                        i += 2;
                        continue;
                    }
                    if chars[i] == '`' {
                        i += 1; // close
                        break;
                    }
                    code.push(chars[i]);
                    i += 1;
                }
                elements.push(InlineElement::Code(code));
                continue;
            }
            // inline math
            if c == '$' {
                if !buffer.is_empty() {
                    elements.push(InlineElement::Text(buffer.clone()));
                    buffer.clear();
                }
                i += 1; // skip opening
                let mut math = String::new();
                while i < chars.len() {
                    if chars[i] == '\\' && i + 1 < chars.len() && chars[i + 1] == '$' {
                        math.push('$');
                        i += 2;
                        continue;
                    }
                    if chars[i] == '$' {
                        i += 1; // close
                        break;
                    }
                    math.push(chars[i]);
                    i += 1;
                }
                elements.push(InlineElement::InlineMath(math));
                continue;
            }
            // link
            if c == '[' {
                if !buffer.is_empty() {
                    elements.push(InlineElement::Text(buffer.clone()));
                    buffer.clear();
                }
                i += 1; // skip '['
                let start = i;
                while i < chars.len() && chars[i] != ']' {
                    i += 1;
                }
                let link_text: String = chars[start..i].iter().collect();
                if i < chars.len() {
                    i += 1;
                } // skip ']'
                if i < chars.len() && chars[i] == '(' {
                    i += 1; // skip '('
                    let url_start = i;
                    while i < chars.len() && chars[i] != ')' {
                        i += 1;
                    }
                    let url: String = chars[url_start..i].iter().collect();
                    if i < chars.len() {
                        i += 1;
                    } // skip ')'
                    elements.push(InlineElement::Link {
                        text: Self::parse_inline_elements(&link_text),
                        url,
                    });
                    continue;
                } else {
                    // not a link, keep as text
                    buffer.push('[');
                    buffer.push_str(&link_text);
                    if i <= chars.len() {
                        buffer.push(']');
                    }
                    continue;
                }
            }
            // emphasis _
            if c == '_' {
                if !buffer.is_empty() {
                    elements.push(InlineElement::Text(buffer.clone()));
                    buffer.clear();
                }
                i += 1; // skip '_'
                let mut emph = String::new();
                while i < chars.len() {
                    if chars[i] == '\\' && i + 1 < chars.len() && chars[i + 1] == '_' {
                        emph.push('_');
                        i += 2;
                        continue;
                    }
                    if chars[i] == '_' {
                        i += 1; // close
                        break;
                    }
                    emph.push(chars[i]);
                    i += 1;
                }
                elements.push(InlineElement::Emphasis(Self::parse_inline_elements(&emph)));
                continue;
            }
            // strong **
            if c == '*' && i + 1 < chars.len() && chars[i + 1] == '*' {
                if !buffer.is_empty() {
                    elements.push(InlineElement::Text(buffer.clone()));
                    buffer.clear();
                }
                i += 2; // skip '**'
                let mut strong = String::new();
                while i < chars.len() {
                    if i + 1 < chars.len() && chars[i] == '*' && chars[i + 1] == '*' {
                        i += 2; // close
                        break;
                    }
                    if chars[i] == '\\' && i + 1 < chars.len() && chars[i + 1] == '*' {
                        strong.push('*');
                        i += 2;
                        continue;
                    }
                    strong.push(chars[i]);
                    i += 1;
                }
                elements.push(InlineElement::Strong(Self::parse_inline_elements(&strong)));
                continue;
            }

            // default
            buffer.push(c);
            i += 1;
        }
        if !buffer.is_empty() {
            elements.push(InlineElement::Text(buffer));
        }
        elements
    }

    fn parse_table(&self, lines: &mut std::iter::Peekable<Lines>) -> Block {
        let mut table_lines: Vec<String> = Vec::new();
        while let Some(&line) = lines.peek() {
            let t = line.trim();
            if t.starts_with("| ") || Regex::new(r"^(\||\s|\-)*$").unwrap().is_match(t) {
                table_lines.push(line.to_string());
                lines.next();
            } else {
                break;
            }
        }
        // Next non-empty line is caption
        let mut caption = Vec::new();
        while let Some(&line) = lines.peek() {
            if line.trim().is_empty() {
                lines.next();
                continue;
            } else {
                caption = Self::parse_inline_elements(line.trim());
                lines.next();
                break;
            }
        }
        // First line is header
        let header_cells =
            parse_table_row_cells(table_lines.first().map(|s| s.as_str()).unwrap_or(""));
        let header = header_cells
            .into_iter()
            .map(|cell| Self::parse_inline_elements(cell.trim()))
            .collect::<Vec<_>>();
        // Middle lines are data (skip first header and separator lines, and any empty/separator lines)
        let mut rows: Vec<Vec<Vec<InlineElement>>> = Vec::new();
        for row in table_lines.into_iter().skip(1) {
            let t = row.trim();
            if Regex::new(r"^(\||\s|\-)*$").unwrap().is_match(t) {
                continue;
            }
            let cells = parse_table_row_cells(&row)
                .into_iter()
                .map(|cell| Self::parse_inline_elements(cell.trim()))
                .collect::<Vec<_>>();
            rows.push(cells);
        }
        Block::Table {
            id_number: self.tables.len(),
            header,
            rows,
            caption,
        }
    }

    fn parse_big_button(&self, lines: &mut std::iter::Peekable<Lines>) -> Block {
        if let Some(line) = lines.next() {
            let trimmed = line.trim();
            if let Some(rest) = trimmed.strip_prefix(":: ") {
                if let Some((text, url)) = rest.rsplit_once(' ') {
                    return Block::BigButton {
                        text: Self::parse_inline_elements(text.trim()),
                        url: url.trim().to_string(),
                    };
                }
            }
        }
        Block::Paragraph(vec![])
    }
}

fn parse_table_row_cells(row: &str) -> Vec<String> {
    row.split('|')
        .map(|s| s.to_string())
        .filter(|s| !s.trim().is_empty())
        .collect::<Vec<_>>()
}
