use std::sync::Arc;

use appearances_catalog::InMemoryAppearanceCatalog;
use appearances_core::{
    AppearanceCatalog, AppearancesCore, IdeaLink, Place, ProjectLink, SceneLink, SectionLink,
    Subject,
};
use appearances_messaging::{
    ForgetDiscardedIdeaListener, ProjectOutlineAppearancesListener, ProjectSceneAppearancesListener,
};
use authorizing::AllowingEverything;
use clock::FixedClock;
use eventsourcing::Agent;
use inbox::{InMemoryInbox, Inboxed};
use messaging::{Listener, Message, RoutingKey};
use outline_contract::{ATTACHED, AttachmentDTO, DETACHED, OutlineEventDTO, SECTION_REMOVED};
use scenes_contract::{DELETED, IDEA_LINKED, IDEA_UNLINKED, IdeaLinkDTO, SceneDeletedDTO};
use serde_json::{Value, json};
use time::OffsetDateTime;
use transacting::InMemoryTransactions;

struct Wired {
    catalog: Arc<InMemoryAppearanceCatalog>,
    outline: Inboxed<ProjectOutlineAppearancesListener>,
    scenes: Inboxed<ProjectSceneAppearancesListener>,
    ideas: Inboxed<ForgetDiscardedIdeaListener>,
}

fn a_workbench() -> Wired {
    let catalog = Arc::new(InMemoryAppearanceCatalog::new());
    let transactions = Arc::new(InMemoryTransactions::new());
    let inbox = Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(
        OffsetDateTime::UNIX_EPOCH,
    ))));
    let appearances = AppearancesCore::new(
        transactions.clone(),
        catalog.clone(),
        Arc::new(AllowingEverything),
    );

    Wired {
        outline: Inboxed::new(
            ProjectOutlineAppearancesListener::new(appearances.clone()),
            transactions.clone(),
            inbox.clone(),
        ),
        scenes: Inboxed::new(
            ProjectSceneAppearancesListener::new(appearances.clone()),
            transactions.clone(),
            inbox.clone(),
        ),
        ideas: Inboxed::new(
            ForgetDiscardedIdeaListener::new(appearances),
            transactions,
            inbox,
        ),
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

    eventpublishing::published(
        routing,
        kind,
        aggregate,
        2,
        event,
        &Agent::System,
        clock::at(1_000),
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
        clock::at(1_000),
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
    // given: no idea placed anywhere
    let wired = a_workbench();

    // when: an idea is noted in a section
    wired
        .hears(&wired.outline, noted("idea_1", "section_1"))
        .await;

    // then: it appears there
    assert_eq!(
        wired.places_of("idea_1").await,
        vec![a_section("section_1")]
    );
}

#[tokio::test]
async fn a_note_moved_between_sections_appears_only_where_it_went() {
    // given: an idea noted in a section
    let wired = a_workbench();
    wired
        .hears(&wired.outline, noted("idea_1", "section_1"))
        .await;

    // when: it is moved to another section
    wired
        .hears(&wired.outline, unnoted("idea_1", "section_1"))
        .await;
    wired
        .hears(&wired.outline, noted("idea_1", "section_2"))
        .await;

    // then: it appears only in the other
    assert_eq!(
        wired.places_of("idea_1").await,
        vec![a_section("section_2")],
        "the outline announces a move as a detach naming the old section, then an attach"
    );
}

#[tokio::test]
async fn a_scene_attached_in_the_outline_is_not_an_appearance() {
    // given: no idea placed anywhere
    let wired = a_workbench();

    // when: a scene is attached to a section
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

    // then: nothing appears there
    assert!(
        wired.places_of("idea_1").await.is_empty(),
        "only ideas are subjects; a scene placed in a section is the book, not a note about it"
    );
}

#[tokio::test]
async fn a_removed_section_takes_its_notes_with_it() {
    // given: two ideas noted in a section
    let wired = a_workbench();
    wired
        .hears(&wired.outline, noted("idea_1", "section_1"))
        .await;
    wired
        .hears(&wired.outline, noted("idea_2", "section_1"))
        .await;

    // when: the section is removed
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

    // then: neither appears anywhere
    assert!(wired.places_of("idea_1").await.is_empty());
    assert!(wired.places_of("idea_2").await.is_empty());
}

#[tokio::test]
async fn an_idea_linked_from_a_scene_appears_there_until_unlinked() {
    // given: no idea placed anywhere
    let wired = a_workbench();

    // when: an idea is linked from a scene
    wired
        .hears(&wired.scenes, linked("idea_1", "scene_1", 1))
        .await;
    // then: it appears there
    assert_eq!(wired.places_of("idea_1").await, vec![a_scene("scene_1")]);

    // when: it is unlinked
    wired
        .hears(&wired.scenes, unlinked("idea_1", "scene_1", 2))
        .await;
    // then: it appears nowhere
    assert!(wired.places_of("idea_1").await.is_empty());
}

#[tokio::test]
async fn a_deleted_scene_takes_every_link_with_it() {
    // given: two ideas linked from a scene
    let wired = a_workbench();
    wired
        .hears(&wired.scenes, linked("idea_1", "scene_1", 1))
        .await;
    wired
        .hears(&wired.scenes, linked("idea_2", "scene_1", 2))
        .await;

    // when: the scene is deleted
    wired.hears(&wired.scenes, deleted("scene_1")).await;

    // then: neither appears anywhere
    assert!(wired.places_of("idea_1").await.is_empty());
    assert!(wired.places_of("idea_2").await.is_empty());
}

#[tokio::test]
async fn a_discarded_idea_appears_nowhere() {
    // given: an idea noted in a section and linked from a scene
    let wired = a_workbench();
    wired
        .hears(&wired.outline, noted("idea_1", "section_1"))
        .await;
    wired
        .hears(&wired.scenes, linked("idea_1", "scene_1", 1))
        .await;
    // and: another noted in the same section
    wired
        .hears(&wired.outline, noted("idea_2", "section_1"))
        .await;

    // when: the first is discarded
    wired.hears(&wired.ideas, discarded("idea_1")).await;

    // then: it appears nowhere
    assert!(wired.places_of("idea_1").await.is_empty());
    // and: the other stays where it was
    assert_eq!(
        wired.places_of("idea_2").await,
        vec![a_section("section_1")],
        "one idea being discarded leaves every other idea where it was"
    );
}

#[tokio::test]
async fn hearing_the_same_message_twice_is_harmless() {
    // given: no idea placed anywhere
    let wired = a_workbench();

    // when: a note and a link are each heard twice
    for _ in 0..2 {
        wired
            .hears(&wired.outline, noted("idea_1", "section_1"))
            .await;
        wired
            .hears(&wired.scenes, linked("idea_1", "scene_1", 1))
            .await;
    }

    // then: the idea appears once in each place
    assert_eq!(
        wired.places_of("idea_1").await,
        vec![a_scene("scene_1"), a_section("section_1")],
        "deliveries are at least once, so a redelivery must not make an idea appear twice"
    );
}

#[tokio::test]
async fn a_note_tried_again_after_it_was_taken_out_stays_out() {
    // given: an idea taken out of a section after a note that failed
    let wired = a_workbench();
    let late = at_version(noted("idea_1", "section_1"), 2);
    wired
        .hears(
            &wired.outline,
            at_version(unnoted("idea_1", "section_1"), 3),
        )
        .await;

    // when: the note is tried again
    wired.hears(&wired.outline, late).await;

    // then: the idea stays out
    assert!(
        wired.places_of("idea_1").await.is_empty(),
        "an attachment that failed and was tried again after its detachment is older news"
    );
}

#[tokio::test]
async fn a_removal_tried_again_after_a_new_note_leaves_the_note() {
    // given: an idea noted in a section, taken out, and noted again, with the taking out failing
    let wired = a_workbench();
    let late = at_version(unnoted("idea_1", "section_1"), 3);
    wired
        .hears(&wired.outline, at_version(noted("idea_1", "section_1"), 2))
        .await;
    wired
        .hears(&wired.outline, at_version(noted("idea_1", "section_1"), 4))
        .await;

    // when: the taking out is tried again
    wired.hears(&wired.outline, late).await;

    // then: the idea stays noted
    assert_eq!(
        wired.places_of("idea_1").await,
        vec![a_section("section_1")]
    );
}

#[tokio::test]
async fn a_note_tried_again_after_its_section_was_removed_stays_gone() {
    // given: a section removed after a note in it failed
    let wired = a_workbench();
    let late = at_version(noted("idea_1", "section_1"), 2);
    wired
        .hears(&wired.outline, at_version(removed("section_1"), 3))
        .await;

    // when: the note is tried again
    wired.hears(&wired.outline, late).await;

    // then: the idea appears nowhere
    assert!(wired.places_of("idea_1").await.is_empty());
}

#[tokio::test]
async fn a_note_tried_again_after_its_idea_was_discarded_stays_gone() {
    // given: an idea discarded after a note of it failed
    let wired = a_workbench();
    let late = noted("idea_1", "section_1");
    wired.hears(&wired.ideas, discarded("idea_1")).await;

    // when: the note is tried again
    wired.hears(&wired.outline, late).await;

    // then: the idea appears nowhere
    assert!(
        wired.places_of("idea_1").await.is_empty(),
        "the discard comes from the idea's stream, so no outline version can outrank it"
    );
}

#[tokio::test]
async fn a_link_tried_again_after_its_scene_was_deleted_stays_gone() {
    // given: a scene deleted after a link from it failed
    let wired = a_workbench();
    let late = linked("idea_1", "scene_1", 1);
    wired.hears(&wired.scenes, deleted("scene_1")).await;

    // when: the link is tried again
    wired.hears(&wired.scenes, late).await;

    // then: the idea appears nowhere
    assert!(wired.places_of("idea_1").await.is_empty());
}

#[tokio::test]
async fn a_link_tried_again_after_it_was_unlinked_stays_unlinked() {
    // given: an idea unlinked from a scene after its link failed
    let wired = a_workbench();
    let late = linked("idea_1", "scene_1", 1);
    wired
        .hears(&wired.scenes, unlinked("idea_1", "scene_1", 2))
        .await;

    // when: the link is tried again
    wired.hears(&wired.scenes, late).await;

    // then: the idea stays unlinked
    assert!(wired.places_of("idea_1").await.is_empty());
}

#[tokio::test]
async fn an_unlink_tried_again_after_a_new_link_leaves_the_link() {
    // given: an idea linked from a scene, unlinked, and linked again, with the unlinking failing
    let wired = a_workbench();
    let late = unlinked("idea_1", "scene_1", 2);
    wired
        .hears(&wired.scenes, linked("idea_1", "scene_1", 1))
        .await;
    wired
        .hears(&wired.scenes, linked("idea_1", "scene_1", 3))
        .await;

    // when: the unlinking is tried again
    wired.hears(&wired.scenes, late).await;

    // then: the idea stays linked
    assert_eq!(wired.places_of("idea_1").await, vec![a_scene("scene_1")]);
}

#[tokio::test]
async fn a_message_no_listener_can_read_is_refused() {
    // given: messages no listener can read
    let wired = a_workbench();
    let nonsense = |routing: &str| {
        Message::opening(
            RoutingKey::parse(routing).expect("a declared routing key is fine"),
            json!({ "nothing": "useful" }),
            clock::at(1_000),
        )
    };

    // then: each listener refuses them
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

fn in_project(mut message: Message, project: &str) -> Message {
    message.payload["context"] = json!({ "project": project });

    message
}

impl Wired {
    async fn project_of(&self, idea: &str) -> Option<ProjectLink> {
        self.catalog
            .project_of(&Subject::Idea(IdeaLink::from(idea)))
            .await
            .expect("asking should succeed")
    }
}

#[tokio::test]
async fn an_idea_noted_in_a_section_or_linked_to_a_scene_is_known_in_its_project() {
    // given: no idea placed anywhere
    let wired = a_workbench();

    // when: an idea is noted in a section of one project and another linked from a scene of another
    wired
        .hears(
            &wired.outline,
            in_project(noted("idea_1", "section_1"), "project_1"),
        )
        .await;
    wired
        .hears(
            &wired.scenes,
            in_project(linked("idea_2", "scene_1", 1), "project_2"),
        )
        .await;

    // then: each is known in its project
    assert_eq!(
        wired.project_of("idea_1").await,
        Some(ProjectLink::from("project_1"))
    );
    assert_eq!(
        wired.project_of("idea_2").await,
        Some(ProjectLink::from("project_2"))
    );
}

#[tokio::test]
async fn a_message_in_no_project_still_places_its_idea_and_names_no_project() {
    // given: no idea placed anywhere
    let wired = a_workbench();

    // when: an idea is noted in a section in no project
    wired
        .hears(&wired.outline, noted("idea_1", "section_1"))
        .await;

    // then: it appears there
    assert_eq!(
        wired.places_of("idea_1").await,
        vec![a_section("section_1")]
    );
    // and: it is known in no project
    assert_eq!(wired.project_of("idea_1").await, None);
}

#[tokio::test]
async fn a_tombstone_is_laid_when_what_it_marks_went() {
    // given: an idea discarded, a section removed and a scene deleted, all at one moment
    let wired = a_workbench();
    wired.hears(&wired.ideas, discarded("idea_1")).await;
    wired.hears(&wired.outline, removed("section_1")).await;
    wired.hears(&wired.scenes, deleted("scene_1")).await;

    // when: tombstones are forgotten before that moment, after it, and after it again
    let appearances = AppearancesCore::new(
        Arc::new(InMemoryTransactions::new()),
        wired.catalog.clone(),
        Arc::new(AllowingEverything),
    );
    let mut forgotten = Vec::new();
    for moment in [clock::at(1_000), clock::at(1_001), clock::at(1_001)] {
        forgotten.push(
            appearances
                .forget_tombstones_laid_before(moment, u64::MAX)
                .await
                .expect("forgetting tombstones should succeed"),
        );
    }

    // then: all three go after it and only once
    assert_eq!(
        forgotten,
        [0, 3, 0],
        "laid when it happened, not when it was heard, and forgotten for good"
    );
}
