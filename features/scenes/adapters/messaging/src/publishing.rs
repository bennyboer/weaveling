use eventsourcing::AggregateContext;
use messaging::{Message, RoutingKey};
use projects_contract::context::PROJECT;
use scenes_contract::{
    CREATED, DELETED, IDEA_LINKED, IDEA_UNLINKED, IdeaLinkDTO, OPENING_CHANGED, OpeningChangedDTO,
    RETITLED, SceneCreatedDTO, SceneDeletedDTO, SceneRetitledDTO, WORDS_COUNTED, WordsCountedDTO,
};
use scenes_core::SceneChange;
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

fn to_payload(dto: &impl serde::Serialize) -> Value {
    serde_json::to_value(dto).expect("an announcement is plain data and cannot fail to serialize")
}

#[cfg(test)]
mod tests {
    use scenes_core::{IdeaLink, ProjectLink, SceneId, SceneTitle};
    use serde_json::json;
    use time::Duration;

    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn a_scene() -> SceneId {
        SceneId::generate(at(1_000))
    }

    #[test]
    fn a_link_is_announced_with_both_ends() {
        let scene = a_scene();

        let told = message_for(
            &SceneChange::IdeaLinked {
                scene,
                project: ProjectLink::from("project_1"),
                idea: IdeaLink::from("idea_1"),
                version: 3,
            },
            at(2_000),
        );

        assert_eq!(told.routing.to_string(), IDEA_LINKED);
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
        assert_eq!(told.occurred_at, at(2_000));
    }

    #[test]
    fn an_unlink_is_announced_under_its_own_key() {
        let told = message_for(
            &SceneChange::IdeaUnlinked {
                scene: a_scene(),
                project: ProjectLink::from("project_1"),
                idea: IdeaLink::from("idea_1"),
                version: 4,
            },
            at(2_000),
        );

        assert_eq!(told.routing.to_string(), IDEA_UNLINKED);
    }

    #[test]
    fn a_deletion_names_the_scene() {
        let scene = a_scene();

        let told = message_for(
            &SceneChange::Deleted {
                scene,
                project: ProjectLink::from("project_1"),
            },
            at(2_000),
        );

        assert_eq!(told.routing.to_string(), DELETED);
        assert_eq!(
            told.payload,
            json!({ "scene": scene.to_string(), "context": { "project": "project_1" } })
        );
    }

    #[test]
    fn a_word_count_is_announced_with_its_scene_s_project() {
        let scene = a_scene();

        let told = message_for(
            &SceneChange::WordsCounted {
                scene,
                project: ProjectLink::from("project_1"),
                words: 42,
            },
            at(2_000),
        );

        assert_eq!(told.routing.to_string(), WORDS_COUNTED);
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
    fn a_scene_s_name_travels_in_three_announcements_each_naming_its_project() {
        let scene = a_scene();
        let project = ProjectLink::from("project_1");

        let created = message_for(
            &SceneChange::Created {
                scene,
                project: project.clone(),
            },
            at(2_000),
        );
        let retitled = message_for(
            &SceneChange::Retitled {
                scene,
                project: project.clone(),
                title: SceneTitle::new("The loom").expect("a plain title"),
                version: 4,
            },
            at(2_000),
        );
        let opened = message_for(
            &SceneChange::OpeningChanged {
                scene,
                project,
                opening: "The loom stood silent.".to_owned(),
            },
            at(2_000),
        );

        assert_eq!(created.routing.to_string(), CREATED);
        assert_eq!(
            serde_json::from_value::<SceneCreatedDTO>(created.payload).expect("reads back"),
            SceneCreatedDTO {
                scene: scene.to_string(),
                project: "project_1".to_owned(),
            }
        );
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
        let scene = a_scene();

        let linked: IdeaLinkDTO = serde_json::from_value(
            message_for(
                &SceneChange::IdeaLinked {
                    scene,
                    project: ProjectLink::from("project_1"),
                    idea: IdeaLink::from("idea_1"),
                    version: 3,
                },
                at(2_000),
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
                at(2_000),
            )
            .payload,
        )
        .expect("a deletion reads back as the contract says");

        assert_eq!(linked.idea, "idea_1");
        assert_eq!(linked.version, 3);
        assert_eq!(deleted.scene, scene.to_string());
    }

    #[test]
    fn every_announcement_carries_its_scene_s_project_as_its_context() {
        let scene = a_scene();
        let project = ProjectLink::from("project_1");
        let idea = IdeaLink::from("idea_1");

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
            SceneChange::Deleted {
                scene,
                project: project.clone(),
            },
        ] {
            let told = message_for(&change, at(2_000));

            assert_eq!(
                eventpublishing::context_in(&told).get(PROJECT),
                Some("project_1"),
                "{}",
                told.routing
            );
        }
    }
}
