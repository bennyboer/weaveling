use readings_core::{Block, CatalogError};

use crate::kept::blocks::{self, KeptBlock};

pub(crate) fn to_json(blocks: &[Block]) -> String {
    let kept: Vec<KeptBlock> = blocks.iter().map(blocks::kept_as).collect();

    serde_json::to_string(&kept).expect("blocks are plain data and cannot fail to serialize")
}

pub(crate) fn from_json(kept: &str) -> Result<Vec<Block>, CatalogError> {
    let kept: Vec<KeptBlock> =
        serde_json::from_str(kept).map_err(|why| CatalogError::Backend(Box::new(why)))?;

    Ok(kept.into_iter().map(blocks::block_of).collect())
}

#[cfg(test)]
mod tests {
    use readings_core::{AssetLink, FigureSize, Marks, Run};

    use super::*;

    #[test]
    fn every_kind_of_block_comes_back_as_it_was_kept() {
        // given: a scene of every kind of block, with marks and a break
        let blocks = vec![
            Block::Heading {
                level: 2,
                runs: vec![Run::plain("At the harbour")],
            },
            Block::Paragraph(vec![
                Run::Text {
                    text: "one by one".to_owned(),
                    marks: Marks {
                        emphasis: true,
                        strong: true,
                        code: true,
                        link: Some("https://example.org".to_owned()),
                    },
                },
                Run::Break,
            ]),
            Block::Quote(vec![Block::Paragraph(vec![Run::plain("Fresh today.")])]),
            Block::List {
                ordered: true,
                items: vec![vec![Block::Paragraph(vec![Run::plain("nets")])]],
            },
            Block::Code("let tide = high;".to_owned()),
            Block::Rule,
            Block::Figure {
                asset: AssetLink::from("asset_frames"),
                size: FigureSize::Medium,
                caption: "The drying frames.".to_owned(),
            },
        ];

        // when: they are written as kept and read back
        let read = from_json(&to_json(&blocks)).expect("what was written reads back");

        // then: they are as they were
        assert_eq!(read, blocks);
    }

    #[test]
    fn plain_text_is_kept_without_the_marks_it_does_not_have() {
        // given: a paragraph of plain text
        let blocks = vec![Block::Paragraph(vec![Run::plain("The boats came in.")])];

        // when: it is written as kept
        let kept = to_json(&blocks);

        // then: it names only what it has, so a book's worth of plain text stays small
        assert_eq!(
            kept,
            r#"[{"kind":"paragraph","runs":[{"kind":"text","text":"The boats came in.","marks":{}}]}]"#
        );
    }
}
