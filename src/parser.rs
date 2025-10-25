use crate::ast::*;
use regex::Regex;
use std::collections::HashMap;
use std::str::Lines;

#[derive(Debug, Default)]
pub struct Parser {
    pub article: Article,
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
            if let Some(stripped) = trimmed.strip_prefix("lang ") {
                language = Some(stripped.to_string());
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
            Block::SectionHeader {
                level,
                id,
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
            if let Some(stripped) = trimmed.strip_prefix("> ") {
                content.push_str(stripped);
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
                    let mut url = String::new();
                    let mut escaped = false;
                    let mut closed = false;
                    while i < chars.len() {
                        let ch = chars[i];
                        if escaped {
                            url.push(ch);
                            escaped = false;
                            i += 1;
                            continue;
                        }
                        if ch == '\\' {
                            escaped = true;
                            i += 1;
                            continue;
                        }
                        if ch == ')' {
                            i += 1; // consume ')'
                            closed = true;
                            break;
                        }
                        url.push(ch);
                        i += 1;
                    }
                    if escaped {
                        // Trailing backslash with no character to escape; keep it literal.
                        url.push('\\');
                    }
                    if !closed {
                        // No closing ')' found; rewind to treat as literal text.
                        buffer.push('[');
                        buffer.push_str(&link_text);
                        buffer.push(']');
                        buffer.push('(');
                        buffer.push_str(&url);
                        continue;
                    }
                    elements.push(InlineElement::Link {
                        text: Self::parse_inline_elements(&link_text),
                        url,
                    });
                    continue;
                } else {
                    let trimmed = link_text.trim();
                    if let Some(name) = trimmed.strip_prefix('#').and_then(|rest| {
                        if is_valid_refname(rest) {
                            Some(rest.to_string())
                        } else {
                            None
                        }
                    }) {
                        elements.push(InlineElement::ReferenceAnchor {
                            content: name,
                            invisible: false,
                        });
                    } else {
                        buffer.push('[');
                        buffer.push_str(&link_text);
                        if i <= chars.len() {
                            buffer.push(']');
                        }
                    }
                    continue;
                }
            }
            if c == '(' && i + 2 < chars.len() && chars[i + 1] == '#' {
                let mut j = i + 2;
                while j < chars.len() && is_valid_refname_char(chars[j]) {
                    j += 1;
                }
                if j > i + 2 && j < chars.len() && chars[j] == ')' {
                    if !buffer.is_empty() {
                        elements.push(InlineElement::Text(buffer.clone()));
                        buffer.clear();
                    }
                    let name: String = chars[i + 2..j].iter().collect();
                    elements.push(InlineElement::Reference(name));
                    i = j + 1;
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
            if t.starts_with("| ") || is_table_separator_row(t) {
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
        let mut header: Vec<Vec<InlineElement>> = Vec::new();
        let mut rows: Vec<Vec<Vec<InlineElement>>> = Vec::new();
        let mut header_filled = false;

        for row in table_lines.into_iter() {
            let t = row.trim();
            if is_table_separator_row(t) {
                continue;
            }
            let cells = parse_table_row_cells(&row)
                .into_iter()
                .map(|cell| Self::parse_inline_elements(cell.trim()))
                .collect::<Vec<_>>();
            if !header_filled {
                header = cells;
                header_filled = true;
            } else {
                rows.push(cells);
            }
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

fn is_table_separator_row(row: &str) -> bool {
    let trimmed = row.trim();
    if trimmed.is_empty() {
        return true;
    }
    trimmed.chars().all(|c| matches!(c, '|' | '-' | ' ' | '\t'))
}

fn is_valid_refname(s: &str) -> bool {
    !s.is_empty() && s.chars().all(is_valid_refname_char)
}

fn is_valid_refname_char(c: char) -> bool {
    c.is_alphanumeric() || matches!(c, '-' | '_')
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
    fn separator_rows_are_ignored() {
        let input = "Table Demo\n\n===\n\n| Colour | Pattern |\n| ------- | -------- |\n| White | Spots |\n";
        let mut parser = Parser::default();
        parser.parse(input);
        let table = parser
            .article
            .body
            .iter()
            .find_map(|block| {
                if let Block::Table { header, rows, .. } = block {
                    Some((header, rows))
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
    }

    #[test]
    fn separator_rows_with_wrong_column_count_are_ignored() {
        let input = "Table Demo\n\n===\n\n| Colour | Pattern |\n|----------------|\n| White | Spots |\n| Black | Solid |\n";
        let mut parser = Parser::default();
        parser.parse(input);
        let table = parser
            .article
            .body
            .iter()
            .find_map(|block| {
                if let Block::Table { rows, .. } = block {
                    Some(rows)
                } else {
                    None
                }
            })
            .expect("expected table rows");
        assert_eq!(table.len(), 2);
        assert!(table.iter().all(|row| row.len() == 2));
    }

    #[test]
    fn parses_reference_citation() {
        let input = "Doc\n\n===\n\nThis cites (#eade).\n";
        let mut parser = Parser::default();
        parser.parse(input);
        let paragraph = parser
            .article
            .body
            .iter()
            .find_map(|block| {
                if let Block::Paragraph(elements) = block {
                    Some(elements)
                } else {
                    None
                }
            })
            .expect("expected paragraph");
        assert!(paragraph
            .iter()
            .any(|el| { matches!(el, InlineElement::Reference(name) if name == "eade") }));
    }

    #[test]
    fn parses_reference_anchor() {
        let input = "Doc\n\n===\n\n[#eade]\n";
        let mut parser = Parser::default();
        parser.parse(input);
        let paragraph = parser
            .article
            .body
            .iter()
            .find_map(|block| {
                if let Block::Paragraph(elements) = block {
                    Some(elements)
                } else {
                    None
                }
            })
            .expect("expected paragraph");
        assert!(paragraph.iter().any(|el| {
            matches!(
                el,
                InlineElement::ReferenceAnchor {
                    content,
                    invisible: false
                } if content == "eade"
            )
        }));
    }
}
