use readings_core::{AssetLink, Block, FigureSize, Marks, Run};
use serde::{Deserialize, Serialize};

/// A block as the databases keep it. Its form is the catalog's own, apart from the API's,
/// so either can change without the other, and it changes only together with a migration.
#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum KeptBlock {
    Paragraph {
        runs: Vec<KeptRun>,
    },
    Heading {
        level: u8,
        runs: Vec<KeptRun>,
    },
    Quote {
        blocks: Vec<KeptBlock>,
    },
    List {
        ordered: bool,
        items: Vec<Vec<KeptBlock>>,
    },
    Code {
        text: String,
    },
    Rule,
    Figure {
        asset: String,
        size: KeptSize,
        caption: String,
    },
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub(crate) enum KeptSize {
    Small,
    Medium,
    Full,
}

#[derive(Debug, Serialize, Deserialize)]
#[serde(tag = "kind", rename_all = "snake_case")]
pub(crate) enum KeptRun {
    Text { text: String, marks: KeptMarks },
    Break,
}

#[derive(Debug, Default, Serialize, Deserialize)]
pub(crate) struct KeptMarks {
    #[serde(default, skip_serializing_if = "is_unset")]
    emphasis: bool,
    #[serde(default, skip_serializing_if = "is_unset")]
    strong: bool,
    #[serde(default, skip_serializing_if = "is_unset")]
    code: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    link: Option<String>,
}

fn is_unset(set: &bool) -> bool {
    !set
}

pub(crate) fn kept_as(block: &Block) -> KeptBlock {
    match block {
        Block::Paragraph(runs) => KeptBlock::Paragraph {
            runs: runs.iter().map(run_kept_as).collect(),
        },
        Block::Heading { level, runs } => KeptBlock::Heading {
            level: *level,
            runs: runs.iter().map(run_kept_as).collect(),
        },
        Block::Quote(blocks) => KeptBlock::Quote {
            blocks: blocks.iter().map(kept_as).collect(),
        },
        Block::List { ordered, items } => KeptBlock::List {
            ordered: *ordered,
            items: items
                .iter()
                .map(|item| item.iter().map(kept_as).collect())
                .collect(),
        },
        Block::Code(text) => KeptBlock::Code { text: text.clone() },
        Block::Rule => KeptBlock::Rule,
        Block::Figure {
            asset,
            size,
            caption,
        } => KeptBlock::Figure {
            asset: asset.to_string(),
            size: match size {
                FigureSize::Small => KeptSize::Small,
                FigureSize::Medium => KeptSize::Medium,
                FigureSize::Full => KeptSize::Full,
            },
            caption: caption.clone(),
        },
    }
}

fn run_kept_as(run: &Run) -> KeptRun {
    match run {
        Run::Text { text, marks } => KeptRun::Text {
            text: text.clone(),
            marks: KeptMarks {
                emphasis: marks.emphasis,
                strong: marks.strong,
                code: marks.code,
                link: marks.link.clone(),
            },
        },
        Run::Break => KeptRun::Break,
    }
}

pub(crate) fn block_of(kept: KeptBlock) -> Block {
    match kept {
        KeptBlock::Paragraph { runs } => Block::Paragraph(runs.into_iter().map(run_of).collect()),
        KeptBlock::Heading { level, runs } => Block::Heading {
            level,
            runs: runs.into_iter().map(run_of).collect(),
        },
        KeptBlock::Quote { blocks } => Block::Quote(blocks.into_iter().map(block_of).collect()),
        KeptBlock::List { ordered, items } => Block::List {
            ordered,
            items: items
                .into_iter()
                .map(|item| item.into_iter().map(block_of).collect())
                .collect(),
        },
        KeptBlock::Code { text } => Block::Code(text),
        KeptBlock::Rule => Block::Rule,
        KeptBlock::Figure {
            asset,
            size,
            caption,
        } => Block::Figure {
            asset: AssetLink::from(asset),
            size: match size {
                KeptSize::Small => FigureSize::Small,
                KeptSize::Medium => FigureSize::Medium,
                KeptSize::Full => FigureSize::Full,
            },
            caption,
        },
    }
}

fn run_of(kept: KeptRun) -> Run {
    match kept {
        KeptRun::Text { text, marks } => Run::Text {
            text,
            marks: Marks {
                emphasis: marks.emphasis,
                strong: marks.strong,
                code: marks.code,
                link: marks.link,
            },
        },
        KeptRun::Break => Run::Break,
    }
}
