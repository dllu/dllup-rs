#[derive(Debug, Default)]
pub struct Article {
    pub header: Option<ArticleHeader>,
    pub body: Vec<Block>,
}

#[derive(Debug)]
pub struct ArticleHeader {
    pub title: String,
    pub date: Option<String>,
}

#[derive(Debug)]
pub enum Block {
    Raw(String),
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    SectionHeader {
        level: usize,
        id: String,
        id_number: usize,
        text: String,
    },
    BlockQuote(Vec<InlineElement>),
    ImageFigure {
        url: String,
        id: Option<String>,
        id_number: usize,
        text: Vec<InlineElement>,
    },
    DisplayMath {
        id: Option<String>,
        id_number: usize,
        content: String,
    },
    UnorderedList(Vec<ListItem>),
    OrderedList(Vec<ListItem>),
    Paragraph(Vec<InlineElement>),
}

#[derive(Debug)]
pub struct ListItem {
    pub level: usize,
    pub text: Vec<InlineElement>,
}

#[derive(Debug)]
pub enum InlineElement {
    Text(String),
    Code(String),
    InlineMath(String),
    Link { text: Vec<InlineElement>, url: String },
    Emphasis(Vec<InlineElement>),
    Strong(Vec<InlineElement>),
    Reference(String),
    ReferenceAnchor { content: String, invisible: bool },
}
