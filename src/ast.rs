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
#[allow(clippy::enum_variant_names)]
pub enum Block {
    Raw(String),
    CodeBlock {
        language: Option<String>,
        code: String,
    },
    SectionHeader {
        level: usize,
        id: String,
        text: String,
    },
    BlockQuote(Vec<InlineElement>),
    ImageFigure {
        url: String,
        id: Option<String>,
        id_number: usize,
        alt: String,
        text: Vec<InlineElement>,
    },
    DisplayMath {
        id: Option<String>,
        id_number: usize,
        content: String,
    },
    Table {
        id_number: usize,
        header: Vec<Vec<InlineElement>>,    // list of header cells
        rows: Vec<Vec<Vec<InlineElement>>>, // list of rows, each row is list of cells
        caption: Vec<InlineElement>,
    },
    BigButton {
        text: Vec<InlineElement>,
        url: String,
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
    Link {
        text: Vec<InlineElement>,
        url: String,
    },
    Emphasis(Vec<InlineElement>),
    Strong(Vec<InlineElement>),
    #[allow(dead_code)]
    Reference(String),
    #[allow(dead_code)]
    ReferenceAnchor {
        content: String,
        invisible: bool,
    },
}
