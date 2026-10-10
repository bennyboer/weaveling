use std::sync::Arc;

use appearances_catalog::InMemoryAppearanceCatalog;
use appearances_core::{AppearanceCatalog, IdeaLink, Place, SceneLink, SectionLink, Subject};
use appearances_messaging::{
    ForgetDiscardedIdea, OutlineAppearancesProjector, SceneAppearancesProjector,
};
use messaging::{Listener, Message, RoutingKey};
use outline_contract::{ATTACHED, AttachmentDTO, DETACHED, OutlineEventDTO, SECTION_REMOVED};
use scenes_contract::{DELETED, IDEA_LINKED, IDEA_UNLINKED, IdeaLinkDTO, SceneDeletedDTO};
use serde_json::{Value, json};
use time::{Duration, OffsetDateTime};

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

struct Wired {
    catalog: Arc<InMemoryAppearanceCatalog>,
    outline: OutlineAppearancesProjector,
    scenes: SceneAppearancesProjector,
    ideas: ForgetDiscardedIdea,
}

fn a_workbench() -> Wired {
    let catalog = Arc::new(InMemoryAppearanceCatalog::new());

    Wired {
        outline: OutlineAppearancesProjector::new(catalog.clone()),
        scenes: SceneAppearancesProjector::new(catalog.clone()),
        ideas: ForgetDiscardedIdea::new(catalog.clone()),
        catalog,
    }
}

impl Wired {
    async fn places_of(&self, idea: &str) -> Vec<Place> {
        self.catalog
            .places_of(&Subject::Idea(IdeaLink::from(idea)))
            .await
            .expect("asking should succeed")
    }

    async fn hears(&self, listener: &impl Listener, message: Message) {
        listener
            .handle(&message)
            .await
            .expect("the listener should take this message");
    }
}

fn a_section(id: &str) -> Place {
    Place::Section(SectionLink::from(id))
}

fn a_scene(id: &str) -> Place {
    Place::Scene(SceneLink::from(id))
}

fn published(routing: &str, aggregate: &str, kind: &str, body: Value) -> Message {
    let mut event = body;
    event["version"] = json!(0);

    Message::opening(
        RoutingKey::parse(routing).expect("a declared routing key is fine"),
        json!({
            "event": event,
            "aggregate": { "id": aggregate, "kind": kind, "version": 2 },
            "agent": { "kind": "system", "id": null },
            "occurred_at": "1970-01-01T00:16:40Z",
        }),
        at(1_000),
    )
}

fn from_the_outline(routing: &str, body: OutlineEventDTO) -> Message {
    published(
        routing,
        "outline_1",
        "outline",
        serde_json::to_value(body).expect("an outline event is plain data"),
    )
}

fn noted(idea: &str, section: &str) -> Message {
    from_the_outline(
        ATTACHED,
        OutlineEventDTO::Attached {
            attachment: AttachmentDTO::Idea {
                id: idea.to_owned(),
            },
            to: section.to_owned(),
            after: None,
        },
    )
}

fn unnoted(idea: &str, section: &str) -> Message {
    from_the_outline(
        DETACHED,
        OutlineEventDTO::Detached {
            attachment: AttachmentDTO::Idea {
                id: idea.to_owned(),
            },
            from: section.to_owned(),
        },
    )
}

fn at_version(mut message: Message, version: u64) -> Message {
    message.payload["aggregate"]["version"] = json!(version);

    message
}

fn removed(section: &str) -> Message {
    from_the_outline(
        SECTION_REMOVED,
        OutlineEventDTO::SectionRemoved {
            section: section.to_owned(),
        },
    )
}

fn from_scenes(routing: &str, payload: Value) -> Message {
    Message::opening(
        RoutingKey::parse(routing).expect("a declared routing key is fine"),
        payload,
        at(1_000),
    )
}

fn linked(idea: &str, scene: &str, version: u64) -> Message {
    from_scenes(
        IDEA_LINKED,
        serde_json::to_value(IdeaLinkDTO {
            scene: scene.to_owned(),
            idea: idea.to_owned(),
            version,
        })
        .expect("a link is plain data"),
    )
}

fn unlinked(idea: &str, scene: &str, version: u64) -> Message {
    from_scenes(
        IDEA_UNLINKED,
        serde_json::to_value(IdeaLinkDTO {
            scene: scene.to_owned(),
            idea: idea.to_owned(),
            version,
        })
        .expect("a link is plain data"),
    )
}

fn deleted(scene: &str) -> Message {
    from_scenes(
        DELETED,
        serde_json::to_value(SceneDeletedDTO {
            scene: scene.to_owned(),
        })
        .expect("a deletion is plain data"),
    )
}

fn discarded(idea: &str) -> Message {
    published(
        ideas_contract::DISCARDED,
        idea,
        "idea",
        json!({ "name": "DISCARDED" }),
    )
}

#[tokio::test]
async fn an_idea_noted_in_a_section_appears_there() {
    let wired = a_workbench();

    wired
        .hears(&wired.outline, noted("idea_1", "section_1"))
        .await;

    assert_eq!(
        wired.places_of("idea_1").await,
        vec![a_section("section_1")]
    );
}

#[tokio::test]
async fn a_note_moved_between_sections_appears_only_where_it_went() {
    let wired = a_workbench();
    wired
        .hears(&wired.outline, noted("idea_1", "section_1"))
        .await;

    wired
        .hears(&wired.outline, unnoted("idea_1", "section_1"))
        .await;
    wired
        .hears(&wired.outline, noted("idea_1", "section_2"))
        .await;

    assert_eq!(
        wired.places_of("idea_1").await,
        vec![a_section("section_2")],
        "the outline announces a move as a detach naming the old section, then an attach"
    );
}

#[tokio::test]
async fn a_scene_attached_in_the_outline_is_not_an_appearance() {
    let wired = a_workbench();

    wired
        .hears(
            &wired.outline,
            from_the_outline(
                ATTACHED,
                OutlineEventDTO::Attached {
                    attachment: AttachmentDTO::Scene {
                        id: "idea_1".to_owned(),
                    },
                    to: "section_1".to_owned(),
                    after: None,
                },
            ),
        )
        .await;

    assert!(
        wired.places_of("idea_1").await.is_empty(),
        "only ideas are subjects; a scene placed in a section is the book, not a note about it"
    );
}

#[tokio::test]
async fn a_removed_section_takes_its_notes_with_it() {
    let wired = a_workbench();
    wired
        .hears(&wired.outline, noted("idea_1", "section_1"))
        .await;
    wired
        .hears(&wired.outline, noted("idea_2", "section_1"))
        .await;

    wired
        .hears(
            &wired.outline,
            from_the_outline(
                SECTION_REMOVED,
                OutlineEventDTO::SectionRemoved {
                    section: "section_1".to_owned(),
                },
            ),
        )
        .await;

    assert!(wired.places_of("idea_1").await.is_empty());
    assert!(wired.places_of("idea_2").await.is_empty());
}

#[tokio::test]
async fn an_idea_linked_from_a_scene_appears_there_until_unlinked() {
    let wired = a_workbench();

    wired
        .hears(&wired.scenes, linked("idea_1", "scene_1", 1))
        .await;
    assert_eq!(wired.places_of("idea_1").await, vec![a_scene("scene_1")]);

    wired
        .hears(&wired.scenes, unlinked("idea_1", "scene_1", 2))
        .await;
    assert!(wired.places_of("idea_1").await.is_empty());
}

#[tokio::test]
async fn a_deleted_scene_takes_every_link_with_it() {
    let wired = a_workbench();
    wired
        .hears(&wired.scenes, linked("idea_1", "scene_1", 1))
        .await;
    wired
        .hears(&wired.scenes, linked("idea_2", "scene_1", 2))
        .await;

    wired.hears(&wired.scenes, deleted("scene_1")).await;

    assert!(wired.places_of("idea_1").await.is_empty());
    assert!(wired.places_of("idea_2").await.is_empty());
}

#[tokio::test]
async fn a_discarded_idea_appears_nowhere() {
    let wired = a_workbench();
    wired
        .hears(&wired.outline, noted("idea_1", "section_1"))
        .await;
    wired
        .hears(&wired.scenes, linked("idea_1", "scene_1", 1))
        .await;
    wired
        .hears(&wired.outline, noted("idea_2", "section_1"))
        .await;

    wired.hears(&wired.ideas, discarded("idea_1")).await;

    assert!(wired.places_of("idea_1").await.is_empty());
    assert_eq!(
        wired.places_of("idea_2").await,
        vec![a_section("section_1")],
        "one idea being discarded leaves every other idea where it was"
    );
}

#[tokio::test]
async fn hearing_the_same_message_twice_is_harmless() {
    let wired = a_workbench();

    for _ in 0..2 {
        wired
            .hears(&wired.outline, noted("idea_1", "section_1"))
            .await;
        wired
            .hears(&wired.scenes, linked("idea_1", "scene_1", 1))
            .await;
    }

    assert_eq!(
        wired.places_of("idea_1").await,
        vec![a_scene("scene_1"), a_section("section_1")],
        "deliveries are at least once, so a redelivery must not make an idea appear twice"
    );
}

#[tokio::test]
async fn a_note_tried_again_after_it_was_taken_out_stays_out() {
    let wired = a_workbench();
    let late = at_version(noted("idea_1", "section_1"), 2);
    wired
        .hears(
            &wired.outline,
            at_version(unnoted("idea_1", "section_1"), 3),
        )
        .await;

    wired.hears(&wired.outline, late).await;

    assert!(
        wired.places_of("idea_1").await.is_empty(),
        "an attachment that failed and was tried again after its detachment is older news"
    );
}

#[tokio::test]
async fn a_removal_tried_again_after_a_new_note_leaves_the_note() {
    let wired = a_workbench();
    let late = at_version(unnoted("idea_1", "section_1"), 3);
    wired
        .hears(&wired.outline, at_version(noted("idea_1", "section_1"), 2))
        .await;
    wired
        .hears(&wired.outline, at_version(noted("idea_1", "section_1"), 4))
        .await;

    wired.hears(&wired.outline, late).await;

    assert_eq!(
        wired.places_of("idea_1").await,
        vec![a_section("section_1")]
    );
}

#[tokio::test]
async fn a_note_tried_again_after_its_section_was_removed_stays_gone() {
    let wired = a_workbench();
    let late = at_version(noted("idea_1", "section_1"), 2);
    wired
        .hears(&wired.outline, at_version(removed("section_1"), 3))
        .await;

    wired.hears(&wired.outline, late).await;

    assert!(wired.places_of("idea_1").await.is_empty());
}

#[tokio::test]
async fn a_note_tried_again_after_its_idea_was_discarded_stays_gone() {
    let wired = a_workbench();
    let late = noted("idea_1", "section_1");
    wired.hears(&wired.ideas, discarded("idea_1")).await;

    wired.hears(&wired.outline, late).await;

    assert!(
        wired.places_of("idea_1").await.is_empty(),
        "the discard comes from the idea's stream, so no outline version can outrank it"
    );
}

#[tokio::test]
async fn a_link_tried_again_after_its_scene_was_deleted_stays_gone() {
    let wired = a_workbench();
    let late = linked("idea_1", "scene_1", 1);
    wired.hears(&wired.scenes, deleted("scene_1")).await;

    wired.hears(&wired.scenes, late).await;

    assert!(wired.places_of("idea_1").await.is_empty());
}

#[tokio::test]
async fn a_link_tried_again_after_it_was_unlinked_stays_unlinked() {
    let wired = a_workbench();
    let late = linked("idea_1", "scene_1", 1);
    wired
        .hears(&wired.scenes, unlinked("idea_1", "scene_1", 2))
        .await;

    wired.hears(&wired.scenes, late).await;

    assert!(wired.places_of("idea_1").await.is_empty());
}

#[tokio::test]
async fn an_unlink_tried_again_after_a_new_link_leaves_the_link() {
    let wired = a_workbench();
    let late = unlinked("idea_1", "scene_1", 2);
    wired
        .hears(&wired.scenes, linked("idea_1", "scene_1", 1))
        .await;
    wired
        .hears(&wired.scenes, linked("idea_1", "scene_1", 3))
        .await;

    wired.hears(&wired.scenes, late).await;

    assert_eq!(wired.places_of("idea_1").await, vec![a_scene("scene_1")]);
}

#[tokio::test]
async fn a_message_no_listener_can_read_is_refused() {
    let wired = a_workbench();
    let nonsense = |routing: &str| {
        Message::opening(
            RoutingKey::parse(routing).expect("a declared routing key is fine"),
            json!({ "nothing": "useful" }),
            at(1_000),
        )
    };

    assert!(wired.outline.handle(&nonsense(ATTACHED)).await.is_err());
    assert!(wired.scenes.handle(&nonsense(IDEA_LINKED)).await.is_err());
    assert!(
        wired
            .ideas
            .handle(&nonsense(ideas_contract::DISCARDED))
            .await
            .is_err(),
        "a message a listener cannot act on belongs in dead letters, not silently dropped"
    );
}
