use readings_contract::{BlockDTO, ContentsDTO, MarksDTO, PartDTO, RunDTO};

use crate::outline::model::SectionId;
use crate::scenes::model::SceneId;

/// A book as a reader sees it: its title, then its sections and scenes in reading order.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Book {
    pub title: String,
    pub parts: Vec<Part>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Part {
    Section {
        section: SectionId,
        depth: u8,
        title: String,
    },
    Scene {
        scene: SceneId,
        blocks: Vec<Block>,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
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
        asset: String,
        size: FigureSize,
        caption: String,
    },
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum FigureSize {
    Small,
    Medium,
    Full,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Run {
    Text { text: String, marks: Marks },
    Break,
}

#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Marks {
    pub emphasis: bool,
    pub strong: bool,
    pub code: bool,
    pub link: Option<String>,
}

impl From<ContentsDTO> for Book {
    fn from(dto: ContentsDTO) -> Self {
        Self {
            title: dto.title,
            parts: dto.parts.into_iter().map(Part::from).collect(),
        }
    }
}

impl From<PartDTO> for Part {
    fn from(dto: PartDTO) -> Self {
        match dto {
            PartDTO::Section {
                section,
                depth,
                title,
            } => Self::Section {
                section: SectionId::from(section),
                depth,
                title,
            },
            PartDTO::Scene { scene, blocks } => Self::Scene {
                scene: SceneId::from(scene),
                blocks: blocks.into_iter().map(Block::from).collect(),
            },
        }
    }
}

impl From<BlockDTO> for Block {
    fn from(dto: BlockDTO) -> Self {
        match dto {
            BlockDTO::Paragraph { runs } => Self::Paragraph(runs_of(runs)),
            BlockDTO::Heading { level, runs } => Self::Heading {
                level,
                runs: runs_of(runs),
            },
            BlockDTO::Quote { blocks } => Self::Quote(blocks_of(blocks)),
            BlockDTO::List { ordered, items } => Self::List {
                ordered,
                items: items.into_iter().map(blocks_of).collect(),
            },
            BlockDTO::Code { text } => Self::Code(text),
            BlockDTO::Rule => Self::Rule,
            BlockDTO::Figure {
                asset,
                size,
                caption,
            } => Self::Figure {
                asset,
                size: FigureSize::named(&size),
                caption,
            },
        }
    }
}

impl FigureSize {
    fn named(told: &str) -> Self {
        match told {
            "small" => Self::Small,
            "medium" => Self::Medium,
            _ => Self::Full,
        }
    }

    pub fn name(self) -> &'static str {
        match self {
            Self::Small => "small",
            Self::Medium => "medium",
            Self::Full => "full",
        }
    }
}

impl From<RunDTO> for Run {
    fn from(dto: RunDTO) -> Self {
        match dto {
            RunDTO::Text { text, marks } => Self::Text {
                text,
                marks: Marks::from(marks),
            },
            RunDTO::Break => Self::Break,
        }
    }
}

impl From<MarksDTO> for Marks {
    fn from(dto: MarksDTO) -> Self {
        Self {
            emphasis: dto.emphasis,
            strong: dto.strong,
            code: dto.code,
            link: dto.link,
        }
    }
}

fn blocks_of(told: Vec<BlockDTO>) -> Vec<Block> {
    told.into_iter().map(Block::from).collect()
}

fn runs_of(told: Vec<RunDTO>) -> Vec<Run> {
    told.into_iter().map(Run::from).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_figure_of_a_size_not_known_is_shown_full() {
        // given: a figure whose size is not one the client knows
        let told = BlockDTO::Figure {
            asset: "asset_1".to_owned(),
            size: "huge".to_owned(),
            caption: String::new(),
        };

        // when: it is read
        let read = Block::from(told);

        // then: it is shown full
        assert_eq!(
            read,
            Block::Figure {
                asset: "asset_1".to_owned(),
                size: FigureSize::Full,
                caption: String::new(),
            }
        );
    }
}
