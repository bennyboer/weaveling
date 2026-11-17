use readings_contract::{BlockDTO, ContentsDTO, MarksDTO, PartDTO, RunDTO};
use readings_core::{Block, Contents, FigureSize, Marks, Part, Run};

pub(crate) fn to_dto(contents: &Contents) -> ContentsDTO {
    ContentsDTO {
        project: contents.project.to_string(),
        title: contents.title.clone(),
        parts: contents.parts.iter().map(to_part_dto).collect(),
    }
}

fn to_part_dto(part: &Part) -> PartDTO {
    match part {
        Part::Section {
            section,
            depth,
            title,
        } => PartDTO::Section {
            section: section.to_string(),
            depth: *depth,
            title: title.clone(),
        },
        Part::Scene { scene, blocks } => PartDTO::Scene {
            scene: scene.to_string(),
            blocks: to_blocks_dto(blocks),
        },
    }
}

fn to_blocks_dto(blocks: &[Block]) -> Vec<BlockDTO> {
    blocks.iter().map(to_block_dto).collect()
}

fn to_block_dto(block: &Block) -> BlockDTO {
    match block {
        Block::Paragraph(runs) => BlockDTO::Paragraph {
            runs: to_runs_dto(runs),
        },
        Block::Heading { level, runs } => BlockDTO::Heading {
            level: *level,
            runs: to_runs_dto(runs),
        },
        Block::Quote(blocks) => BlockDTO::Quote {
            blocks: to_blocks_dto(blocks),
        },
        Block::List { ordered, items } => BlockDTO::List {
            ordered: *ordered,
            items: items.iter().map(|item| to_blocks_dto(item)).collect(),
        },
        Block::Code(text) => BlockDTO::Code { text: text.clone() },
        Block::Rule => BlockDTO::Rule,
        Block::Figure {
            asset,
            size,
            caption,
        } => BlockDTO::Figure {
            asset: asset.to_string(),
            size: size_name(*size).to_owned(),
            caption: caption.clone(),
        },
    }
}

fn size_name(size: FigureSize) -> &'static str {
    match size {
        FigureSize::Small => "small",
        FigureSize::Medium => "medium",
        FigureSize::Full => "full",
    }
}

fn to_runs_dto(runs: &[Run]) -> Vec<RunDTO> {
    runs.iter().map(to_run_dto).collect()
}

fn to_run_dto(run: &Run) -> RunDTO {
    match run {
        Run::Text { text, marks } => RunDTO::Text {
            text: text.clone(),
            marks: to_marks_dto(marks),
        },
        Run::Break => RunDTO::Break,
    }
}

fn to_marks_dto(marks: &Marks) -> MarksDTO {
    MarksDTO {
        emphasis: marks.emphasis,
        strong: marks.strong,
        code: marks.code,
        link: marks.link.clone(),
    }
}
