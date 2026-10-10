use messaging::{Message, RoutingKey};
use scenes_contract::{DELETED, IDEA_LINKED, IDEA_UNLINKED, IdeaLinkDTO, SceneDeletedDTO};
use scenes_core::SceneChange;
use serde_json::Value;
use time::OffsetDateTime;

pub fn message_for(change: &SceneChange, at: OffsetDateTime) -> Message {
    let (routing, payload) = match change {
        SceneChange::IdeaLinked {
            scene,
            idea,
            version,
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
        } => (
            IDEA_UNLINKED,
            to_payload(&IdeaLinkDTO {
                scene: scene.to_string(),
                idea: idea.to_string(),
                version: *version,
            }),
        ),
        SceneChange::Deleted { scene } => (
            DELETED,
            to_payload(&SceneDeletedDTO {
                scene: scene.to_string(),
            }),
        ),
    };

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
    use scenes_core::{IdeaLink, SceneId};
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
                idea: IdeaLink::from("idea_1"),
                version: 3,
            },
            at(2_000),
        );

        assert_eq!(told.routing.to_string(), IDEA_LINKED);
        assert_eq!(
            told.payload,
            json!({ "scene": scene.to_string(), "idea": "idea_1", "version": 3 }),
            "a listener cannot ask scenes which idea it was, so both ends travel in the message,              and the version tells a late link from a newer unlink"
        );
        assert_eq!(told.occurred_at, at(2_000));
    }

    #[test]
    fn an_unlink_is_announced_under_its_own_key() {
        let told = message_for(
            &SceneChange::IdeaUnlinked {
                scene: a_scene(),
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

        let told = message_for(&SceneChange::Deleted { scene }, at(2_000));

        assert_eq!(told.routing.to_string(), DELETED);
        assert_eq!(told.payload, json!({ "scene": scene.to_string() }));
    }

    #[test]
    fn every_announcement_reads_back_as_its_contract_shape() {
        let scene = a_scene();

        let linked: IdeaLinkDTO = serde_json::from_value(
            message_for(
                &SceneChange::IdeaLinked {
                    scene,
                    idea: IdeaLink::from("idea_1"),
                    version: 3,
                },
                at(2_000),
            )
            .payload,
        )
        .expect("a link reads back as the contract says");
        let deleted: SceneDeletedDTO =
            serde_json::from_value(message_for(&SceneChange::Deleted { scene }, at(2_000)).payload)
                .expect("a deletion reads back as the contract says");

        assert_eq!(linked.idea, "idea_1");
        assert_eq!(linked.version, 3);
        assert_eq!(deleted.scene, scene.to_string());
    }
}
