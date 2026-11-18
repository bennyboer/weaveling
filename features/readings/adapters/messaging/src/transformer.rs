use outline_contract::{AttachmentDTO, SectionStateDTO};
use readings_core::{
    AssetLink, Block, FigureSize, Marks, Run, SceneLink, SectionLink, ToldSection,
};
use scenes_contract::{BlockDTO, MarksDTO, RunDTO};

/// The sections as the outline told them, with only the scenes they hold, as a book shows no notes.
pub(crate) fn to_told_sections(told: &[SectionStateDTO]) -> Vec<ToldSection> {
    told.iter()
        .map(|section| ToldSection {
            section: SectionLink::from(section.section.as_str()),
            parent: section.parent.as_deref().map(SectionLink::from),
            title: section.title.clone(),
            scenes: section
                .attachments
                .iter()
                .filter_map(|attachment| match attachment {
                    AttachmentDTO::Scene { id } => Some(SceneLink::from(id.as_str())),
                    AttachmentDTO::Idea { .. } => None,
                })
                .collect(),
        })
        .collect()
}

pub(crate) fn to_blocks(told: &[BlockDTO]) -> Vec<Block> {
    told.iter().map(to_block).collect()
}

fn to_block(told: &BlockDTO) -> Block {
    match told {
        BlockDTO::Paragraph { runs } => Block::Paragraph(to_runs(runs)),
        BlockDTO::Heading { level, runs } => Block::Heading {
            level: *level,
            runs: to_runs(runs),
        },
        BlockDTO::Quote { blocks } => Block::Quote(to_blocks(blocks)),
        BlockDTO::List { ordered, items } => Block::List {
            ordered: *ordered,
            items: items.iter().map(|item| to_blocks(item)).collect(),
        },
        BlockDTO::Code { text } => Block::Code(text.clone()),
        BlockDTO::Rule => Block::Rule,
        BlockDTO::Figure {
            asset,
            size,
            caption,
        } => Block::Figure {
            asset: AssetLink::from(asset.as_str()),
            size: to_size(size),
            caption: caption.clone(),
        },
    }
}

fn to_size(told: &str) -> FigureSize {
    match told {
        "small" => FigureSize::Small,
        "medium" => FigureSize::Medium,
        _ => FigureSize::Full,
    }
}

fn to_runs(told: &[RunDTO]) -> Vec<Run> {
    told.iter()
        .map(|run| match run {
            RunDTO::Text { text, marks } => Run::Text {
                text: text.clone(),
                marks: to_marks(marks),
            },
            RunDTO::Break => Run::Break,
        })
        .collect()
}

fn to_marks(told: &MarksDTO) -> Marks {
    Marks {
        emphasis: told.emphasis,
        strong: told.strong,
        code: told.code,
        link: told.link.clone(),
    }
}
