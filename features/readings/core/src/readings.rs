use eventsourcing::Agent;

use crate::contents::{Block, Contents, Marks, Part, Run};
use crate::error::ReadingsError;
use crate::link::{ProjectLink, SceneLink, SectionLink};

#[derive(Clone, Default)]
pub struct ReadingsCore;

impl ReadingsCore {
    pub fn new() -> Self {
        Self
    }

    /// The book as it is written now, put together from the outline and the scenes.
    pub async fn live_book(
        &self,
        project: &str,
        _agent: &Agent,
    ) -> Result<Contents, ReadingsError> {
        // TODO: members only, and the book put together from the outline and the scenes behind ports
        Ok(sample_book(project))
    }
}

fn sample_book(project: &str) -> Contents {
    Contents {
        project: ProjectLink::from(project),
        title: "The Loom".to_owned(),
        parts: vec![
            Part::Section {
                section: SectionLink::from("section_morning"),
                depth: 0,
                title: "Morning".to_owned(),
            },
            Part::Scene {
                scene: SceneLink::from("scene_harbour"),
                blocks: vec![
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
                ],
            },
            Part::Section {
                section: SectionLink::from("section_market"),
                depth: 1,
                title: "The market".to_owned(),
            },
            Part::Scene {
                scene: SceneLink::from("scene_stalls"),
                blocks: vec![
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
                ],
            },
        ],
    }
}
