use std::collections::HashMap;

use async_trait::async_trait;
use readings_core::{
    Block, BookOutline, CatalogError, Marks, OutlineCatalog, OutlinedSection, ProjectLink, Run,
    SceneLink, SceneTextCatalog, SectionLink,
};

/// The same small book for every project, until the catalogs are kept from the outline's and the scenes' messages.
pub struct SampleBook;

#[async_trait]
impl OutlineCatalog for SampleBook {
    async fn outline_of(
        &self,
        _project: &ProjectLink,
    ) -> Result<Option<BookOutline>, CatalogError> {
        // TODO: kept from the outline's messages
        Ok(Some(BookOutline {
            title: "The Loom".to_owned(),
            sections: vec![
                OutlinedSection {
                    section: SectionLink::from("section_morning"),
                    depth: 0,
                    title: "Morning".to_owned(),
                    scenes: vec![
                        SceneLink::from("scene_harbour"),
                        SceneLink::from("scene_wall"),
                    ],
                },
                OutlinedSection {
                    section: SectionLink::from("section_market"),
                    depth: 1,
                    title: "The market".to_owned(),
                    scenes: vec![SceneLink::from("scene_stalls")],
                },
            ],
        }))
    }
}

#[async_trait]
impl SceneTextCatalog for SampleBook {
    async fn texts_of(
        &self,
        scenes: &[SceneLink],
    ) -> Result<HashMap<SceneLink, Vec<Block>>, CatalogError> {
        // TODO: kept from the scenes' messages
        Ok(scenes
            .iter()
            .filter_map(|scene| text_of(scene.as_str()).map(|text| (scene.clone(), text)))
            .collect())
    }
}

fn text_of(scene: &str) -> Option<Vec<Block>> {
    match scene {
        "scene_harbour" => Some(vec![
            Block::Paragraph(vec![
                Run::plain("The boats came in before the light did, "),
                Run::Text {
                    text: "one by one".to_owned(),
                    marks: Marks {
                        emphasis: true,
                        ..Marks::default()
                    },
                },
                Run::plain(", their lamps swinging."),
            ]),
            Block::Paragraph(vec![Run::plain(
                "Hedy counted them from the wall and lost count twice.",
            )]),
        ]),
        "scene_wall" => Some(vec![Block::Paragraph(vec![Run::plain(
            "By the time the last lamp was in, she had stopped pretending she was counting at all.",
        )])]),
        "scene_stalls" => Some(vec![
            Block::Quote(vec![Block::Paragraph(vec![Run::plain(
                "Fresh today, fresh tomorrow.",
            )])]),
            Block::Rule,
            Block::Paragraph(vec![
                Run::plain("Nobody believed the sign."),
                Run::Break,
                Run::Text {
                    text: "Everybody bought the fish.".to_owned(),
                    marks: Marks {
                        strong: true,
                        ..Marks::default()
                    },
                },
            ]),
        ]),
        _ => None,
    }
}
