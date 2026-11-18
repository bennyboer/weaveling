use yrs::any::Number;
use yrs::types::Attrs;
use yrs::types::text::YChange;
use yrs::types::xml::{XmlElementRef, XmlOut, XmlTextRef};
use yrs::{Any, Doc, Out, ReadTxn, Text, Transact, Xml, XmlFragment};

use crate::projection::{self, FRAGMENT};

/// A part of a scene's text as a reader sees it.
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
        asset: String,
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

/// The marks a reader sees. Quotation marks and speech are left out:
/// the quotation marks are in the text already, and speech is only for the writing view.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Marks {
    pub emphasis: bool,
    pub strong: bool,
    pub code: bool,
    pub link: Option<String>,
}

const LOWEST_HEADING: u8 = 6;

pub(crate) fn blocks_of(doc: &Doc) -> Vec<Block> {
    let fragment = doc.get_or_insert_xml_fragment(FRAGMENT);
    let txn = doc.transact();

    blocks_among(fragment.children(&txn), &txn)
}

fn blocks_among<T: ReadTxn>(nodes: impl Iterator<Item = XmlOut>, txn: &T) -> Vec<Block> {
    let mut blocks = Vec::new();
    for node in nodes {
        match node {
            XmlOut::Element(element) => blocks.extend(block_of(&element, txn)),
            XmlOut::Fragment(fragment) => blocks.extend(blocks_among(fragment.children(txn), txn)),
            XmlOut::Text(text) => blocks.push(Block::Paragraph(merged(runs_of_text(&text, txn)))),
        }
    }

    blocks
}

/// Reads one element as the blocks it stands for.
/// An element this does not know is read as the blocks it holds, so nothing written in it is lost.
fn block_of<T: ReadTxn>(element: &XmlElementRef, txn: &T) -> Vec<Block> {
    let children = || element.children(txn);

    match element.tag().as_ref() {
        "paragraph" => vec![Block::Paragraph(runs_among(children(), txn))],
        "heading" => vec![Block::Heading {
            level: level_of(element, txn),
            runs: runs_among(children(), txn),
        }],
        "blockquote" => vec![Block::Quote(blocks_among(children(), txn))],
        "bullet_list" | "ordered_list" => vec![Block::List {
            ordered: element.tag().as_ref() == "ordered_list",
            items: children()
                .map(|item| match item {
                    XmlOut::Element(item) => blocks_among(item.children(txn), txn),
                    other => blocks_among(std::iter::once(other), txn),
                })
                .collect(),
        }],
        "code_block" => vec![Block::Code(plain_among(children(), txn))],
        "horizontal_rule" => vec![Block::Rule],
        "figure" => figure_of(element, txn).into_iter().collect(),
        _ => blocks_among(children(), txn),
    }
}

fn level_of<T: ReadTxn>(element: &XmlElementRef, txn: &T) -> u8 {
    let level = match element.get_attribute(txn, "level") {
        Some(Out::Any(Any::Number(Number::Int(level)))) => level,
        Some(Out::Any(Any::Number(Number::Float(level)))) => level as i64,
        Some(Out::Any(Any::String(level))) => level.parse().unwrap_or(1),
        _ => 1,
    };

    level.clamp(1, i64::from(LOWEST_HEADING)) as u8
}

/// Reads a figure, if it names an asset. A figure naming none shows nothing to a reader.
fn figure_of<T: ReadTxn>(element: &XmlElementRef, txn: &T) -> Option<Block> {
    let Some(Out::Any(Any::String(asset))) = element.get_attribute(txn, "asset") else {
        return None;
    };
    if !projection::is_asset_id(&asset) {
        return None;
    }
    let size = match element.get_attribute(txn, "size") {
        Some(Out::Any(Any::String(size))) if size.as_ref() == "small" => FigureSize::Small,
        Some(Out::Any(Any::String(size))) if size.as_ref() == "medium" => FigureSize::Medium,
        _ => FigureSize::Full,
    };

    Some(Block::Figure {
        asset: asset.to_string(),
        size,
        caption: plain_among(element.children(txn), txn),
    })
}

fn runs_among<T: ReadTxn>(nodes: impl Iterator<Item = XmlOut>, txn: &T) -> Vec<Run> {
    let mut runs = Vec::new();
    for node in nodes {
        match node {
            XmlOut::Text(text) => runs.extend(runs_of_text(&text, txn)),
            XmlOut::Element(element) if element.tag().as_ref() == "hard_break" => {
                runs.push(Run::Break);
            }
            XmlOut::Element(element) => runs.extend(runs_among(element.children(txn), txn)),
            XmlOut::Fragment(fragment) => runs.extend(runs_among(fragment.children(txn), txn)),
        }
    }

    merged(runs)
}

fn runs_of_text<T: ReadTxn>(text: &XmlTextRef, txn: &T) -> Vec<Run> {
    text.diff(txn, YChange::identity)
        .into_iter()
        .filter_map(|chunk| match chunk.insert {
            Out::Any(Any::String(written)) => Some(Run::Text {
                text: written.to_string(),
                marks: chunk
                    .attributes
                    .map(|attrs| marks_of(&attrs))
                    .unwrap_or_default(),
            }),
            _ => None,
        })
        .collect()
}

fn marks_of(attrs: &Attrs) -> Marks {
    Marks {
        emphasis: attrs.contains_key("em"),
        strong: attrs.contains_key("strong"),
        code: attrs.contains_key("code"),
        link: match attrs.get("link") {
            Some(Any::Map(link)) => match link.get("href") {
                Some(Any::String(href)) => Some(href.to_string()),
                _ => None,
            },
            _ => None,
        },
    }
}

/// Joins neighbouring runs that look the same to a reader,
/// as runs apart only by a mark the reader does not see.
fn merged(runs: Vec<Run>) -> Vec<Run> {
    let mut joined: Vec<Run> = Vec::with_capacity(runs.len());
    for run in runs {
        match (joined.last_mut(), run) {
            (
                Some(Run::Text { text, marks }),
                Run::Text {
                    text: more,
                    marks: same,
                },
            ) if *marks == same => text.push_str(&more),
            (_, run) => joined.push(run),
        }
    }

    joined
}

fn plain_among<T: ReadTxn>(nodes: impl Iterator<Item = XmlOut>, txn: &T) -> String {
    runs_among(nodes, txn)
        .into_iter()
        .map(|run| match run {
            Run::Text { text, .. } => text,
            Run::Break => "\n".to_owned(),
        })
        .collect()
}
