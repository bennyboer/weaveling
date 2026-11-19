use eventsourcing::AggregateContext;
use messaging::{Message, RoutingKey};
use projects_contract::context::PROJECT;
use scenes_contract::{
    ASSETS_CHANGED, BlockDTO, CREATED, DELETED, IDEA_LINKED, IDEA_UNLINKED, IdeaLinkDTO, MarksDTO,
    OPENING_CHANGED, OpeningChangedDTO, RETITLED, RunDTO, SceneAssetsChangedDTO, SceneCreatedDTO,
    SceneDeletedDTO, SceneRetitledDTO, SceneTextStateDTO, TEXT_STATE_CHANGED, WORDS_COUNTED,
    WordsCountedDTO,
};
use scenes_core::{Block, FigureSize, Marks, Run, SceneChange};
use serde_json::Value;
use time::OffsetDateTime;

pub fn message_for(change: &SceneChange, at: OffsetDateTime) -> Message {
    let (routing, payload) = match change {
        SceneChange::IdeaLinked {
            scene,
            idea,
            version,
            ..
        } => (
            IDEA_LINKED,
            to_payload(&IdeaLinkDTO {
                scene: scene.to_string(),
                idea: idea.to_string(),
                version: *version,
            }),
        ),
        SceneChange::IdeaUnlinked {
            scene,
            idea,
            version,
            ..
        } => (
            IDEA_UNLINKED,
            to_payload(&IdeaLinkDTO {
                scene: scene.to_string(),
                idea: idea.to_string(),
                version: *version,
            }),
        ),
        SceneChange::Deleted { scene, .. } => (
            DELETED,
            to_payload(&SceneDeletedDTO {
                scene: scene.to_string(),
            }),
        ),
        SceneChange::Created { scene, project } => (
            CREATED,
            to_payload(&SceneCreatedDTO {
                scene: scene.to_string(),
                project: project.to_string(),
            }),
        ),
        SceneChange::Retitled {
            scene,
            project,
            title,
            version,
        } => (
            RETITLED,
            to_payload(&SceneRetitledDTO {
                scene: scene.to_string(),
                project: project.to_string(),
                title: title.to_string(),
                version: *version,
            }),
        ),
        SceneChange::OpeningChanged {
            scene,
            project,
            opening,
        } => (
            OPENING_CHANGED,
            to_payload(&OpeningChangedDTO {
                scene: scene.to_string(),
                project: project.to_string(),
                opening: opening.clone(),
            }),
        ),
        SceneChange::WordsCounted {
            scene,
            project,
            words,
        } => (
            WORDS_COUNTED,
            to_payload(&WordsCountedDTO {
                scene: scene.to_string(),
                project: project.to_string(),
                words: *words as u64,
            }),
        ),
        SceneChange::AssetsChanged {
            scene,
            assets,
            version,
            ..
        } => (
            ASSETS_CHANGED,
            to_payload(&SceneAssetsChangedDTO {
                scene: scene.to_string(),
                assets: assets.iter().cloned().collect(),
                version: *version,
            }),
        ),
        SceneChange::TextSettled {
            scene,
            blocks,
            version,
            ..
        } => (
            TEXT_STATE_CHANGED,
            to_payload(&SceneTextStateDTO {
                scene: scene.to_string(),
                blocks: to_blocks_dto(blocks),
                version: *version,
            }),
        ),
    };

    let mut payload = payload;
    payload["context"] = AggregateContext::default()
        .with(PROJECT, change.project().to_string())
        .written();

    Message::opening(
        RoutingKey::parse(routing).expect("a declared routing key holds no wildcards"),
        payload,
        at,
    )
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
            asset: asset.clone(),
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
    runs.iter()
        .map(|run| match run {
            Run::Text { text, marks } => RunDTO::Text {
                text: text.clone(),
                marks: to_marks_dto(marks),
            },
            Run::Break => RunDTO::Break,
        })
        .collect()
}

fn to_marks_dto(marks: &Marks) -> MarksDTO {
    MarksDTO {
        emphasis: marks.emphasis,
        strong: marks.strong,
        code: marks.code,
        link: marks.link.clone(),
    }
}

fn to_payload(dto: &impl serde::Serialize) -> Value {
    serde_json::to_value(dto).expect("an announcement is plain data and cannot fail to serialize")
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use scenes_core::{IdeaLink, ProjectLink, SceneId, SceneTitle};
    use serde_json::json;

    use super::*;

    fn a_scene() -> SceneId {
        SceneId::generate(clock::at(1_000))
    }

    #[test]
    fn a_link_is_announced_with_both_ends() {
        // given: a scene
        let scene = a_scene();

        // when: its link to an idea is announced
        let told = message_for(
            &SceneChange::IdeaLinked {
                scene,
                project: ProjectLink::from("project_1"),
                idea: IdeaLink::from("idea_1"),
                version: 3,
            },
            clock::at(2_000),
        );

        // then: it goes under the link's key
        assert_eq!(told.routing.to_string(), IDEA_LINKED);
        // and: names both ends, the version and the project
        assert_eq!(
            told.payload,
            json!({
                "scene": scene.to_string(),
                "idea": "idea_1",
                "version": 3,
                "context": { "project": "project_1" },
            }),
            "a listener cannot ask scenes which idea it was, so both ends travel in the message,              and the version tells a late link from a newer unlink"
        );
        // and: when it happened
        assert_eq!(told.occurred_at, clock::at(2_000));
    }

    #[test]
    fn an_unlink_is_announced_under_its_own_key() {
        // when: an unlink is announced
        let told = message_for(
            &SceneChange::IdeaUnlinked {
                scene: a_scene(),
                project: ProjectLink::from("project_1"),
                idea: IdeaLink::from("idea_1"),
                version: 4,
            },
            clock::at(2_000),
        );

        // then: it goes under the unlink's key
        assert_eq!(told.routing.to_string(), IDEA_UNLINKED);
    }

    #[test]
    fn a_deletion_names_the_scene() {
        // given: a scene
        let scene = a_scene();

        // when: its deletion is announced
        let told = message_for(
            &SceneChange::Deleted {
                scene,
                project: ProjectLink::from("project_1"),
            },
            clock::at(2_000),
        );

        // then: it goes under the deletion's key
        assert_eq!(told.routing.to_string(), DELETED);
        // and: names the scene and its project
        assert_eq!(
            told.payload,
            json!({ "scene": scene.to_string(), "context": { "project": "project_1" } })
        );
    }

    #[test]
    fn a_word_count_is_announced_with_its_scene_s_project() {
        // given: a scene
        let scene = a_scene();

        // when: its word count is announced
        let told = message_for(
            &SceneChange::WordsCounted {
                scene,
                project: ProjectLink::from("project_1"),
                words: 42,
            },
            clock::at(2_000),
        );

        // then: it goes under the count's key
        assert_eq!(told.routing.to_string(), WORDS_COUNTED);
        // and: reads back as the count of the scene in its project
        let counted: WordsCountedDTO =
            serde_json::from_value(told.payload).expect("a count reads back as its contract");
        assert_eq!(
            counted,
            WordsCountedDTO {
                scene: scene.to_string(),
                project: "project_1".to_owned(),
                words: 42,
            },
            "statistics are kept per project, so the count names its project rather than              making a listener ask scenes for it"
        );
    }

    #[test]
    fn the_images_a_scene_holds_are_announced_sorted_with_the_version_they_were_read_at() {
        // given: a scene
        let scene = a_scene();

        // when: the assets it holds are announced
        let told = message_for(
            &SceneChange::AssetsChanged {
                scene,
                project: ProjectLink::from("project_1"),
                assets: BTreeSet::from(["asset_weft".to_owned(), "asset_loom".to_owned()]),
                version: 7,
            },
            clock::at(2_000),
        );

        // then: it goes under the assets' key
        assert_eq!(told.routing.to_string(), ASSETS_CHANGED);
        // and: reads back as the assets of the scene, sorted, with their version
        assert_eq!(
            serde_json::from_value::<SceneAssetsChangedDTO>(told.payload).expect("reads back"),
            SceneAssetsChangedDTO {
                scene: scene.to_string(),
                assets: vec!["asset_loom".to_owned(), "asset_weft".to_owned()],
                version: 7,
            },
            "the version tells a late, older set from a newer one"
        );
    }

    #[test]
    fn a_scene_s_settled_text_is_announced_as_blocks_with_the_version_it_was_read_at() {
        // given: a scene
        let scene = a_scene();

        // when: its text is announced, a heading, a marked paragraph with a break, and a figure
        let told = message_for(
            &SceneChange::TextSettled {
                scene,
                project: ProjectLink::from("project_1"),
                blocks: vec![
                    Block::Heading {
                        level: 2,
                        runs: vec![Run::Text {
                            text: "At the harbour".to_owned(),
                            marks: Marks::default(),
                        }],
                    },
                    Block::Paragraph(vec![
                        Run::Text {
                            text: "one by one".to_owned(),
                            marks: Marks {
                                emphasis: true,
                                ..Marks::default()
                            },
                        },
                        Run::Break,
                    ]),
                    Block::Figure {
                        asset: "asset_frames".to_owned(),
                        size: FigureSize::Medium,
                        caption: "The drying frames.".to_owned(),
                    },
                ],
                version: 9,
            },
            clock::at(2_000),
        );

        // then: it goes under the text's key
        assert_eq!(told.routing.to_string(), TEXT_STATE_CHANGED);
        // and: reads back as the scene's blocks, with their version
        assert_eq!(
            serde_json::from_value::<SceneTextStateDTO>(told.payload.clone()).expect("reads back"),
            SceneTextStateDTO {
                scene: scene.to_string(),
                blocks: vec![
                    BlockDTO::Heading {
                        level: 2,
                        runs: vec![RunDTO::Text {
                            text: "At the harbour".to_owned(),
                            marks: MarksDTO::default(),
                        }],
                    },
                    BlockDTO::Paragraph {
                        runs: vec![
                            RunDTO::Text {
                                text: "one by one".to_owned(),
                                marks: MarksDTO {
                                    emphasis: true,
                                    ..MarksDTO::default()
                                },
                            },
                            RunDTO::Break,
                        ],
                    },
                    BlockDTO::Figure {
                        asset: "asset_frames".to_owned(),
                        size: "medium".to_owned(),
                        caption: "The drying frames.".to_owned(),
                    },
                ],
                version: 9,
            },
            "the version tells a late, older text from a newer one"
        );
        // and: names the scene's project in its context
        assert_eq!(told.payload["context"]["project"], "project_1");
    }

    #[test]
    fn a_scene_s_name_travels_in_three_announcements_each_naming_its_project() {
        // given: a scene in a project
        let scene = a_scene();
        let project = ProjectLink::from("project_1");

        // when: its creation, retitling and new opening are announced
        let created = message_for(
            &SceneChange::Created {
                scene,
                project: project.clone(),
            },
            clock::at(2_000),
        );
        let retitled = message_for(
            &SceneChange::Retitled {
                scene,
                project: project.clone(),
                title: SceneTitle::new("The loom").expect("a plain title"),
                version: 4,
            },
            clock::at(2_000),
        );
        let opened = message_for(
            &SceneChange::OpeningChanged {
                scene,
                project,
                opening: "The loom stood silent.".to_owned(),
            },
            clock::at(2_000),
        );

        // then: the creation reads back with its project
        assert_eq!(created.routing.to_string(), CREATED);
        assert_eq!(
            serde_json::from_value::<SceneCreatedDTO>(created.payload).expect("reads back"),
            SceneCreatedDTO {
                scene: scene.to_string(),
                project: "project_1".to_owned(),
            }
        );
        // and: the retitling reads back with its project, title and version
        assert_eq!(retitled.routing.to_string(), RETITLED);
        assert_eq!(
            serde_json::from_value::<SceneRetitledDTO>(retitled.payload).expect("reads back"),
            SceneRetitledDTO {
                scene: scene.to_string(),
                project: "project_1".to_owned(),
                title: "The loom".to_owned(),
                version: 4,
            }
        );
        // and: the new opening reads back with its project
        assert_eq!(opened.routing.to_string(), OPENING_CHANGED);
        assert_eq!(
            serde_json::from_value::<OpeningChangedDTO>(opened.payload).expect("reads back"),
            OpeningChangedDTO {
                scene: scene.to_string(),
                project: "project_1".to_owned(),
                opening: "The loom stood silent.".to_owned(),
            }
        );
    }

    #[test]
    fn every_announcement_reads_back_as_its_contract_shape() {
        // given: a scene
        let scene = a_scene();

        // when: its link and its deletion are announced and read back
        let linked: IdeaLinkDTO = serde_json::from_value(
            message_for(
                &SceneChange::IdeaLinked {
                    scene,
                    project: ProjectLink::from("project_1"),
                    idea: IdeaLink::from("idea_1"),
                    version: 3,
                },
                clock::at(2_000),
            )
            .payload,
        )
        .expect("a link reads back as the contract says");
        let deleted: SceneDeletedDTO = serde_json::from_value(
            message_for(
                &SceneChange::Deleted {
                    scene,
                    project: ProjectLink::from("project_1"),
                },
                clock::at(2_000),
            )
            .payload,
        )
        .expect("a deletion reads back as the contract says");

        // then: each says what was announced
        assert_eq!(linked.idea, "idea_1");
        assert_eq!(linked.version, 3);
        assert_eq!(deleted.scene, scene.to_string());
    }

    #[test]
    fn every_announcement_carries_its_scene_s_project_as_its_context() {
        // given: a scene in a project
        let scene = a_scene();
        let project = ProjectLink::from("project_1");
        let idea = IdeaLink::from("idea_1");

        // when: each change of it is announced
        for change in [
            SceneChange::Created {
                scene,
                project: project.clone(),
            },
            SceneChange::Retitled {
                scene,
                project: project.clone(),
                title: SceneTitle::new("Chapter One").expect("a plain title"),
                version: 2,
            },
            SceneChange::OpeningChanged {
                scene,
                project: project.clone(),
                opening: "It was".to_owned(),
            },
            SceneChange::IdeaLinked {
                scene,
                project: project.clone(),
                idea: idea.clone(),
                version: 3,
            },
            SceneChange::IdeaUnlinked {
                scene,
                project: project.clone(),
                idea: idea.clone(),
                version: 4,
            },
            SceneChange::WordsCounted {
                scene,
                project: project.clone(),
                words: 42,
            },
            SceneChange::AssetsChanged {
                scene,
                project: project.clone(),
                assets: BTreeSet::from(["asset_loom".to_owned()]),
                version: 5,
            },
            SceneChange::Deleted {
                scene,
                project: project.clone(),
            },
        ] {
            let told = message_for(&change, clock::at(2_000));

            // then: it carries the project as its context
            assert_eq!(
                eventpublishing::context_in(&told).get(PROJECT),
                Some("project_1"),
                "{}",
                told.routing
            );
        }
    }
}
