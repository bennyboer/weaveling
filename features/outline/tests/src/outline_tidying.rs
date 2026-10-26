use std::sync::Arc;
use std::sync::atomic::Ordering;

use clock::FixedClock;
use eventsourcing::{AccountId, Agent, AggregateId, EventStore, Version};
use ideas_contract::DISCARDED as IDEA_DISCARDED;
use messaging::{Message, RoutingKey};
use outline_contract::{
    ATTACHED, DETACHED, SECTION_ADDED, SECTION_MOVED, SECTION_REMOVED, STARTED,
};
use outline_core::{
    Attachment, KIND, OutlineCatalog, OutlineEvent, OutlineId, SectionId, SectionTitle,
};
use scenes_contract::{DELETED as SCENE_DELETED, SceneDeletedDTO};
use serde_json::json;
use time::{Duration, OffsetDateTime};

use crate::wiring;
use crate::wiring::{TheSystemAndTheAuthor, Wired};

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn an_author() -> Agent {
    Agent::Account(AccountId::from("author-7"))
}

fn a_workbench() -> Wired {
    a_workbench_guarded_by(Arc::new(TheSystemAndTheAuthor::default()))
}

fn a_workbench_guarded_by(authorizer: Arc<TheSystemAndTheAuthor>) -> Wired {
    wiring::wired_guarded_by(Arc::new(FixedClock::new(at(1_000))), authorizer)
}

fn a_scene(named: &str) -> Attachment {
    Attachment::scene(named)
}

fn an_idea(named: &str) -> Attachment {
    Attachment::idea(named)
}

fn discarded(idea: &str) -> Message {
    Message::opening(
        RoutingKey::parse(IDEA_DISCARDED).expect("a declared routing key is fine"),
        json!({
            "event": { "version": 0, "name": "DISCARDED" },
            "aggregate": { "id": idea, "kind": "idea", "version": 2 },
            "agent": { "kind": "system", "id": null },
            "occurred_at": "1970-01-01T00:16:40Z",
        }),
        at(1_000),
    )
}

fn titled(what: &str) -> SectionTitle {
    SectionTitle::new(what).expect("the title should be usable")
}

async fn a_chapter(wired: &Wired, outline: &OutlineId, titled_as: &str) -> SectionId {
    wired
        .outlines
        .add(
            &outline.to_string(),
            None,
            None,
            titled(titled_as),
            None,
            &an_author(),
        )
        .await
        .expect("adding a section should succeed")
        .section
}

async fn a_book_holding(wired: &Wired, scenes: &[&str]) -> (OutlineId, SectionId) {
    let outline = wired
        .outlines
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed")
        .id;
    let chapter = a_chapter(wired, &outline, "Chapter 1").await;

    let mut behind = None;
    for scene in scenes {
        wired
            .outlines
            .attach(
                &outline.to_string(),
                a_scene(scene),
                chapter,
                behind.clone(),
                None,
                &an_author(),
            )
            .await
            .expect("attaching should succeed");
        behind = Some(a_scene(scene));
    }

    wired.settle().await;

    (outline, chapter)
}

#[tokio::test]
async fn attaching_a_scene_puts_it_in_the_index() {
    let wired = a_workbench();

    let (outline, _) = a_book_holding(&wired, &["scene_1"]).await;

    assert_eq!(
        wired
            .catalog
            .outlines_holding(&a_scene("scene_1"))
            .await
            .expect("looking should succeed"),
        vec![outline]
    );
}

#[tokio::test]
async fn detaching_a_scene_takes_it_out_of_the_index() {
    let wired = a_workbench();
    let (outline, _) = a_book_holding(&wired, &["scene_1"]).await;

    wired
        .outlines
        .detach(&outline.to_string(), a_scene("scene_1"), None, &an_author())
        .await
        .expect("detaching should succeed");
    wired.settle().await;

    assert!(
        wired
            .catalog
            .outlines_holding(&a_scene("scene_1"))
            .await
            .expect("looking should succeed")
            .is_empty()
    );
}

#[tokio::test]
async fn removing_a_section_takes_its_scenes_out_of_the_index() {
    let wired = a_workbench();
    let (outline, chapter) = a_book_holding(&wired, &["scene_1"]).await;

    wired
        .outlines
        .remove(&outline.to_string(), chapter, None, &an_author())
        .await
        .expect("removing should succeed");
    wired.settle().await;

    assert!(
        wired
            .catalog
            .outlines_holding(&a_scene("scene_1"))
            .await
            .expect("looking should succeed")
            .is_empty(),
        "a removed section returns its scenes to the pool, and the index has to hear about it"
    );
}

#[tokio::test]
async fn the_index_hears_everything_that_changes_what_the_book_holds() {
    let wired = a_workbench();

    for changing in [ATTACHED, DETACHED, SECTION_REMOVED] {
        assert!(
            wired
                .indexer
                .hears(&RoutingKey::parse(changing).expect("a declared key is fine")),
            "{changing} changes which scenes are in the book, so the index must hear it"
        );
    }
    for quiet in [STARTED, SECTION_ADDED, SECTION_MOVED] {
        assert!(
            !wired
                .indexer
                .hears(&RoutingKey::parse(quiet).expect("a declared key is fine")),
            "{quiet} rearranges structure without changing the set, so it must not cost a write"
        );
    }
}

#[tokio::test]
async fn the_catalog_projector_hears_only_a_book_being_started() {
    let wired = a_workbench();

    assert!(
        wired
            .projector
            .hears(&RoutingKey::parse(STARTED).expect("a declared key is fine"))
    );
    for quiet in [SECTION_ADDED, SECTION_MOVED, ATTACHED] {
        assert!(
            !wired
                .projector
                .hears(&RoutingKey::parse(quiet).expect("a declared key is fine")),
            "which outline a project has cannot change when {quiet} happens"
        );
    }
}

async fn a_book_noting(wired: &Wired, idea: &str) -> (OutlineId, SectionId) {
    let (outline, chapter) = a_book_holding(wired, &["scene_1"]).await;

    wired
        .outlines
        .attach(
            &outline.to_string(),
            an_idea(idea),
            chapter,
            Some(a_scene("scene_1")),
            None,
            &an_author(),
        )
        .await
        .expect("noting should succeed");
    wired.settle().await;

    (outline, chapter)
}

#[tokio::test]
async fn a_note_is_in_the_index_so_its_idea_can_find_it_again() {
    let wired = a_workbench();
    let (outline, _) = a_book_noting(&wired, "idea_1").await;

    let holding = wired
        .catalog
        .outlines_holding(&an_idea("idea_1"))
        .await
        .expect("looking should succeed");

    assert_eq!(
        holding,
        vec![outline],
        "the index is fed the whole section, not the reading order, or a note would be \
         invisible to the one listener that has to clean it up"
    );
}

#[tokio::test]
async fn a_discarded_idea_leaves_the_book() {
    let wired = a_workbench();
    let (outline, chapter) = a_book_noting(&wired, "idea_1").await;

    wired
        .on_idea_discarded
        .handle(&discarded("idea_1"))
        .await
        .expect("tidying should succeed");

    let standing = wired
        .outlines
        .read(&outline.to_string(), &an_author())
        .await
        .expect("reading should succeed");
    assert_eq!(
        standing.state.attachments_in(&chapter),
        vec![a_scene("scene_1")],
        "a note about an idea nobody kept is a reference to nothing"
    );
}

#[tokio::test]
async fn discarding_an_idea_leaves_the_scenes_alone() {
    let wired = a_workbench();
    let (outline, chapter) = a_book_noting(&wired, "idea_1").await;

    wired
        .on_idea_discarded
        .handle(&discarded("idea_1"))
        .await
        .expect("tidying should succeed");

    let standing = wired
        .outlines
        .read(&outline.to_string(), &an_author())
        .await
        .expect("reading should succeed");
    assert_eq!(
        standing.state.reading_order().len(),
        1,
        "ideas are disposable and scenes are the book, so throwing one away must never \
         reach the other"
    );
    assert!(!standing.state.attachments_in(&chapter).is_empty());
}

#[tokio::test]
async fn hearing_the_same_discard_twice_is_harmless() {
    let wired = a_workbench();
    a_book_noting(&wired, "idea_1").await;

    for _ in 0..2 {
        wired
            .on_idea_discarded
            .handle(&discarded("idea_1"))
            .await
            .expect("a redelivery is not a failure");
    }
}

#[tokio::test]
async fn discarding_an_idea_no_book_ever_noted_is_harmless() {
    let wired = a_workbench();
    a_book_noting(&wired, "idea_1").await;

    wired
        .on_idea_discarded
        .handle(&discarded("idea_elsewhere"))
        .await
        .expect("an idea in no book is nothing to do");
}

async fn attached_as_published(wired: &Wired, outline: &OutlineId) -> Message {
    let recorded = wired
        .store
        .read_from(&AggregateId::from(outline), KIND, Version::ZERO)
        .await
        .expect("reading should succeed")
        .into_iter()
        .find(|recorded| matches!(recorded.event, OutlineEvent::Attached { .. }))
        .expect("something was attached to the book");

    outline_messaging::message_for(&recorded).expect("an attachment is published")
}

#[tokio::test]
async fn a_deleted_scene_is_let_go_of_in_the_system_s_name() {
    let wired = a_workbench();
    let (outline, _) = a_book_holding(&wired, &["scene_1"]).await;

    wired
        .on_scene_deleted
        .handle(&scene_deleted("scene_1"))
        .await
        .expect("tidying should succeed");

    let recorded = wired
        .store
        .read_from(&AggregateId::from(&outline), KIND, Version::ZERO)
        .await
        .expect("reading should succeed");
    assert_eq!(
        recorded
            .last()
            .expect("the detach is recorded")
            .metadata
            .agent,
        Agent::System
    );
}

#[tokio::test]
async fn whom_to_let_in_not_being_known_fails_letting_go_so_it_is_tried_again() {
    let authorizer = Arc::new(TheSystemAndTheAuthor::default());
    let wired = a_workbench_guarded_by(authorizer.clone());
    a_book_holding(&wired, &["scene_1"]).await;
    authorizer.down.store(true, Ordering::Release);

    assert!(
        wired
            .on_scene_deleted
            .handle(&scene_deleted("scene_1"))
            .await
            .is_err(),
        "a deleted scene left in the outline for good is worse than a message tried again"
    );
}

#[tokio::test]
async fn a_late_attachment_does_not_bring_back_what_a_discarded_book_held() {
    let wired = a_workbench();
    let (outline, _) = a_book_holding(&wired, &["scene_1"]).await;
    let late = attached_as_published(&wired, &outline).await;
    wired
        .outlines
        .sweep("project_1")
        .await
        .expect("sweeping should succeed");
    wired.settle().await;

    wired
        .indexer
        .handle(&late)
        .await
        .expect("an attachment tried again after the discard is not a failure");

    assert!(
        wired
            .catalog
            .outlines_holding(&a_scene("scene_1"))
            .await
            .expect("looking should succeed")
            .is_empty(),
        "a discarded book still lists what it held, so reading it back must not put that in the index again"
    );
}

#[tokio::test]
async fn discarding_an_idea_still_indexed_in_a_discarded_book_is_harmless() {
    let wired = a_workbench();
    let (outline, _) = a_book_holding(&wired, &[]).await;
    wired
        .outlines
        .sweep("project_1")
        .await
        .expect("sweeping should succeed");
    wired.settle().await;
    wired
        .catalog
        .holds(outline, &[an_idea("idea_1")])
        .await
        .expect("holding should succeed");

    wired
        .on_idea_discarded
        .handle(&discarded("idea_1"))
        .await
        .expect(
            "a discarded book has nothing left to detach, so this must not end in dead letters",
        );
}

#[tokio::test]
async fn a_discard_that_is_not_a_published_event_is_refused() {
    let wired = a_workbench();
    let nonsense = Message::opening(
        RoutingKey::parse(IDEA_DISCARDED).expect("a declared routing key is fine"),
        json!({ "nothing": "useful" }),
        at(1_000),
    );

    assert!(
        wired.on_idea_discarded.handle(&nonsense).await.is_err(),
        "a message the listener cannot act on belongs in dead letters, not silently dropped"
    );
}

fn scene_deleted(scene: &str) -> Message {
    Message::opening(
        RoutingKey::parse(SCENE_DELETED).expect("a declared routing key is fine"),
        serde_json::to_value(SceneDeletedDTO {
            scene: scene.to_owned(),
        })
        .expect("a deletion is plain data"),
        at(1_000),
    )
}

#[tokio::test]
async fn a_deleted_scene_is_taken_out_of_the_book() {
    let wired = a_workbench();
    let (outline, chapter) = a_book_holding(&wired, &["scene_1", "scene_2"]).await;

    wired
        .on_scene_deleted
        .handle(&scene_deleted("scene_1"))
        .await
        .expect("taking it out should succeed");

    let standing = wired
        .outlines
        .read(&outline.to_string(), &an_author())
        .await
        .expect("reading should succeed");
    assert_eq!(
        standing.state.attachments_in(&chapter),
        vec![a_scene("scene_2")],
        "a book that still held a deleted scene would point at nothing"
    );
}

async fn placed_but_not_yet_indexed(wired: &Wired, held: Attachment) -> (OutlineId, SectionId) {
    let outline = wired
        .outlines
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed")
        .id;
    let chapter = a_chapter(wired, &outline, "Chapter 1").await;
    wired
        .outlines
        .attach(
            &outline.to_string(),
            held,
            chapter,
            None,
            None,
            &an_author(),
        )
        .await
        .expect("attaching should succeed");

    (outline, chapter)
}

#[tokio::test]
async fn a_scene_deleted_before_its_placement_was_indexed_still_leaves_the_book() {
    let wired = a_workbench();
    let (outline, chapter) = placed_but_not_yet_indexed(&wired, a_scene("scene_1")).await;

    wired
        .on_scene_deleted
        .handle(&scene_deleted("scene_1"))
        .await
        .expect("hearing the deletion first should succeed");
    wired.settle().await;

    let standing = wired
        .outlines
        .read(&outline.to_string(), &an_author())
        .await
        .expect("reading should succeed");
    assert!(
        standing.state.attachments_in(&chapter).is_empty(),
        "the index learns of the placement late, and must let go of what is already gone"
    );
}

#[tokio::test]
async fn an_idea_discarded_before_its_placement_was_indexed_still_leaves_the_book() {
    let wired = a_workbench();
    let (outline, chapter) = placed_but_not_yet_indexed(&wired, an_idea("idea_1")).await;

    wired
        .on_idea_discarded
        .handle(&discarded("idea_1"))
        .await
        .expect("hearing the discard first should succeed");
    wired.settle().await;

    let standing = wired
        .outlines
        .read(&outline.to_string(), &an_author())
        .await
        .expect("reading should succeed");
    assert!(
        standing.state.attachments_in(&chapter).is_empty(),
        "the index learns of the placement late, and must let go of what is already gone"
    );
}

#[tokio::test]
async fn hearing_the_same_scene_deletion_twice_is_harmless() {
    let wired = a_workbench();
    a_book_holding(&wired, &["scene_1"]).await;

    for _ in 0..2 {
        wired
            .on_scene_deleted
            .handle(&scene_deleted("scene_1"))
            .await
            .expect("a redelivery is not a failure");
    }
}

#[tokio::test]
async fn deleting_a_scene_no_book_held_is_harmless() {
    let wired = a_workbench();
    a_book_holding(&wired, &["scene_1"]).await;

    wired
        .on_scene_deleted
        .handle(&scene_deleted("scene_never_placed"))
        .await
        .expect("a scene outside every book is not a failure");
}

#[tokio::test]
async fn a_scene_deletion_that_is_not_shaped_like_one_is_refused() {
    let wired = a_workbench();
    let nonsense = Message::opening(
        RoutingKey::parse(SCENE_DELETED).expect("a declared routing key is fine"),
        json!({ "nothing": "useful" }),
        at(1_000),
    );

    assert!(wired.on_scene_deleted.handle(&nonsense).await.is_err());
}

#[tokio::test]
async fn the_scene_deletion_listener_hears_only_deletions() {
    let wired = a_workbench();

    assert!(
        wired
            .on_scene_deleted
            .hears(&RoutingKey::parse(SCENE_DELETED).expect("a plain key is fine"))
    );
    assert!(
        !wired
            .on_scene_deleted
            .hears(&RoutingKey::parse("scene.idea.linked").expect("a plain key is fine")),
        "linking an idea leaves a scene where it is in the book"
    );
}

#[tokio::test]
async fn a_tombstone_is_laid_when_the_idea_was_discarded_or_the_scene_deleted() {
    let wired = a_workbench();
    wired
        .on_idea_discarded
        .handle(&discarded("idea_1"))
        .await
        .expect("the listener should take this message");
    wired
        .on_scene_deleted
        .handle(&scene_deleted("scene_1"))
        .await
        .expect("the listener should take this message");

    for (before, forgotten) in [(at(1_000), 0), (at(1_001), 2)] {
        assert_eq!(
            wired
                .catalog
                .forget_tombstones_laid_before(before, u64::MAX)
                .await
                .expect("forgetting tombstones should succeed"),
            forgotten,
            "laid when it happened"
        );
    }
}
