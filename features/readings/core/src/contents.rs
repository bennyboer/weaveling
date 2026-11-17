use crate::link::{AssetLink, ProjectLink, SceneLink, SectionLink};

/// The text of a book as a reader sees it, in reading order.
/// Sections and scenes follow each other flat; a section's depth says how far it is nested.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Contents {
    pub project: ProjectLink,
    pub title: String,
    pub parts: Vec<Part>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Part {
    Section {
        section: SectionLink,
        depth: u8,
        title: String,
    },
    Scene {
        scene: SceneLink,
        blocks: Vec<Block>,
    },
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Block {
    Paragraph(Vec<Run>),
    Heading {
        level: u8,
        runs: Vec<Run>,
    },
    Quote(Vec<Block>),
    List {
        ordered: bool,
        items: Vec<Vec<Block>>,
    },
    Code(String),
    Rule,
    Figure {
        asset: AssetLink,
        size: FigureSize,
        caption: String,
    },
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FigureSize {
    Small,
    Medium,
    Full,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Run {
    Text { text: String, marks: Marks },
    Break,
}

impl Run {
    pub fn plain(text: &str) -> Self {
        Self::Text {
            text: text.to_owned(),
            marks: Marks::default(),
        }
    }
}

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Marks {
    pub emphasis: bool,
    pub strong: bool,
    pub code: bool,
    pub link: Option<String>,
}
