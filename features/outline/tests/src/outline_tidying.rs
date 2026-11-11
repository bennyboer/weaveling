use std::sync::Arc;
use std::sync::atomic::Ordering;

use clock::FixedClock;
use eventsourcing::{Agent, AggregateId, EventStore, Version};
use ideas_contract::DISCARDED as IDEA_DISCARDED;
use inbox::{InMemoryInbox, Inboxed};
use messaging::{Listener, Message, RoutingKey};
use outline_contract::{
    ATTACHED, AttachmentDTO, DETACHED, SECTION_ADDED, SECTION_MOVED, SECTION_REMOVED, STARTED,
};
use outline_core::{
    Attachment, KIND, OutlineCatalog, OutlineEvent, OutlineId, SectionId, SectionTitle,
};
use outline_messaging::{
    DETACH_EVERYWHERE, DETACH_THE_GONE, DetachEverywhereDTO, DetachOnSceneDeletedListener,
    DetachTheGoneDTO,
};
use scenes_contract::{DELETED as SCENE_DELETED, SceneDeletedDTO};
use serde_json::json;
use transacting::{InMemoryTransactions, Transactions};

use crate::wiring;
use crate::wiring::{TheSystemAndTheAuthor, Wired};

fn a_workbench() -> Wired {
    a_workbench_guarded_by(Arc::new(TheSystemAndTheAuthor::default()))
}

fn a_workbench_guarded_by(authorizer: Arc<TheSystemAndTheAuthor>) -> Wired {
    wiring::wired_guarded_by(Arc::new(FixedClock::new(clock::at(1_000))), authorizer)
}

fn a_scene(named: &str) -> Attachment {
    Attachment::scene(named)
}

fn an_idea(named: &str) -> Attachment {
    Attachment::idea(named)
}

fn discarded(idea: &str) -> Message {
    eventpublishing::published(
        IDEA_DISCARDED,
        "idea",
        idea,
        2,
        json!({ "version": 0, "name": "DISCARDED" }),
        &Agent::System,
        clock::at(1_000),
    )
}

fn asked_to_detach_everywhere(attachment: AttachmentDTO) -> Message {
    Message::opening(
        RoutingKey::parse(DETACH_EVERYWHERE).expect("a follow-up's key holds no wildcards"),
        serde_json::to_value(DetachEverywhereDTO { attachment })
            .expect("a follow-up is plain data"),
        clock::at(1_000),
    )
}

fn asked_to_detach_the_gone(outline: &OutlineId) -> Message {
    Message::opening(
        RoutingKey::parse(DETACH_THE_GONE).expect("a follow-up's key holds no wildcards"),
        serde_json::to_value(DetachTheGoneDTO {
            outline: outline.to_string(),
        })
        .expect("a follow-up is plain data"),
        clock::at(1_000),
    )
}

fn a_title(what: &str) -> SectionTitle {
    SectionTitle::new(what).expect("the title should be usable")
}

async fn a_chapter(wired: &Wired, outline: &OutlineId, titled_as: &str) -> SectionId {
    wired
        .outlines
        .add(
            &outline.to_string(),
            None,
            None,
            a_title(titled_as),
            None,
            &eventsourcing::an_author(),
        )
        .await
        .expect("adding a section should succeed")
        .section
}

async fn a_book_holding(wired: &Wired, scenes: &[&str]) -> (OutlineId, SectionId) {
    let outline = wired
        .outlines
        .open("project_1", &eventsourcing::an_author())
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
                &eventsourcing::an_author(),
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
    // given: no book holding a scene
    let wired = a_workbench();

    // when: a scene is attached to a book
    let (outline, _) = a_book_holding(&wired, &["scene_1"]).await;

    // then: the index finds the book by the scene
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
    // given: a book holding a scene
    let wired = a_workbench();
    let (outline, _) = a_book_holding(&wired, &["scene_1"]).await;

    // when: the scene is detached
    wired
        .outlines
        .detach(
            &outline.to_string(),
            a_scene("scene_1"),
            None,
            &eventsourcing::an_author(),
        )
        .await
        .expect("detaching should succeed");
    wired.settle().await;

    // then: the index finds no book by it
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
    // given: a book holding a scene in a chapter
    let wired = a_workbench();
    let (outline, chapter) = a_book_holding(&wired, &["scene_1"]).await;

    // when: the chapter is removed
    wired
        .outlines
        .remove(
            &outline.to_string(),
            chapter,
            None,
            &eventsourcing::an_author(),
        )
        .await
        .expect("removing should succeed");
    wired.settle().await;

    // then: the index finds no book by the scene
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
    // expect: the index hears everything that changes what a book holds
    let wired = a_workbench();

    for changing in [ATTACHED, DETACHED, SECTION_REMOVED] {
        assert!(
            wired
                .indexer
                .hears(&RoutingKey::parse(changing).expect("a declared key is fine")),
            "{changing} changes which scenes are in the book, so the index must hear it"
        );
    }
    // and: nothing that only rearranges it
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
    // expect: the catalog hears a book being started
    let wired = a_workbench();

    assert!(
        wired
            .projector
            .hears(&RoutingKey::parse(STARTED).expect("a declared key is fine"))
    );
    // and: nothing else
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
            &eventsourcing::an_author(),
        )
        .await
        .expect("noting should succeed");
    wired.settle().await;

    (outline, chapter)
}

#[tokio::test]
async fn a_note_is_in_the_index_so_its_idea_can_find_it_again() {
    // given: a book noting an idea
    let wired = a_workbench();
    let (outline, _) = a_book_noting(&wired, "idea_1").await;

    // when: books holding the idea are looked for
    let holding = wired
        .catalog
        .outlines_holding(&an_idea("idea_1"))
        .await
        .expect("looking should succeed");

    // then: the book is found
    assert_eq!(
        holding,
        vec![outline],
        "the index is fed the whole section, not the reading order, or a note would be \
         invisible to the one listener that has to clean it up"
    );
}

#[tokio::test]
async fn a_discarded_idea_leaves_the_book() {
    // given: a book noting an idea beside a scene
    let wired = a_workbench();
    let (outline, chapter) = a_book_noting(&wired, "idea_1").await;

    // when: the idea's discard is heard
    wired
        .on_idea_discarded
        .handle(&discarded("idea_1"))
        .await
        .expect("tidying should succeed");
    wired.settle().await;

    // then: the book holds only the scene
    let standing = wired
        .outlines
        .read(&outline.to_string(), &eventsourcing::an_author())
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
    // given: a book noting an idea beside a scene
    let wired = a_workbench();
    let (outline, chapter) = a_book_noting(&wired, "idea_1").await;

    // when: the idea's discard is heard
    wired
        .on_idea_discarded
        .handle(&discarded("idea_1"))
        .await
        .expect("tidying should succeed");
    wired.settle().await;

    // then: the book still reads the scene
    let standing = wired
        .outlines
        .read(&outline.to_string(), &eventsourcing::an_author())
        .await
        .expect("reading should succeed");
    assert_eq!(
        standing.state.reading_order().len(),
        1,
        "ideas are disposable and scenes are the book, so throwing one away must never \
         reach the other"
    );
    // and: its chapter still holds it
    assert!(!standing.state.attachments_in(&chapter).is_empty());
}

#[tokio::test]
async fn hearing_the_same_discard_twice_is_harmless() {
    // given: a book noting an idea
    let wired = a_workbench();
    a_book_noting(&wired, "idea_1").await;

    // then: hearing its discard twice succeeds each time
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
    // given: a book noting an idea
    let wired = a_workbench();
    a_book_noting(&wired, "idea_1").await;

    // then: hearing the discard of another idea succeeds
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
    // given: a book holding a scene
    let wired = a_workbench();
    let (outline, _) = a_book_holding(&wired, &["scene_1"]).await;

    // when: the scene's deletion is heard
    wired
        .on_scene_deleted
        .handle(&scene_deleted("scene_1"))
        .await
        .expect("tidying should succeed");
    wired.settle().await;

    // then: the book lets go of it in the system's name
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
    // given: a book holding a scene
    let authorizer = Arc::new(TheSystemAndTheAuthor::default());
    let wired = a_workbench_guarded_by(authorizer.clone());
    a_book_holding(&wired, &["scene_1"]).await;
    // and: whom to let in cannot be known
    authorizer.down.store(true, Ordering::Release);

    // then: letting go of the scene everywhere fails
    assert!(
        wired
            .detaching_everywhere
            .handle(&asked_to_detach_everywhere(AttachmentDTO::Scene {
                id: "scene_1".to_owned()
            }))
            .await
            .is_err(),
        "a deleted scene left in the outline for good is worse than a message tried again"
    );
}

#[tokio::test]
async fn a_late_attachment_does_not_bring_back_what_a_discarded_book_held() {
    // given: a book holding a scene
    let wired = a_workbench();
    let (outline, _) = a_book_holding(&wired, &["scene_1"]).await;
    // and: its attaching as published
    let late = attached_as_published(&wired, &outline).await;
    // and: the book discarded with its project
    wired
        .outlines
        .sweep("project_1")
        .await
        .expect("sweeping should succeed");
    wired.settle().await;

    // when: the attaching is heard late
    wired
        .indexer
        .handle(&late)
        .await
        .expect("an attachment tried again after the discard is not a failure");

    // then: the index finds no book by the scene
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
    // given: a discarded book
    let wired = a_workbench();
    let (outline, _) = a_book_holding(&wired, &[]).await;
    wired
        .outlines
        .sweep("project_1")
        .await
        .expect("sweeping should succeed");
    wired.settle().await;
    // and: an idea still indexed in it
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    wired
        .catalog
        .holds(&mut transaction, outline, u64::MAX, &[an_idea("idea_1")])
        .await
        .expect("holding should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    // then: hearing the idea's discard and letting go of it everywhere succeed
    wired
        .on_idea_discarded
        .handle(&discarded("idea_1"))
        .await
        .expect("laying the tombstone should succeed");
    wired
        .detaching_everywhere
        .handle(&asked_to_detach_everywhere(AttachmentDTO::Idea {
            id: "idea_1".to_owned(),
        }))
        .await
        .expect(
            "a discarded book has nothing left to detach, so this must not end in dead letters",
        );
}

#[tokio::test]
async fn a_discard_that_is_not_a_published_event_is_refused() {
    // given: a discard that is no published event
    let wired = a_workbench();
    let nonsense = Message::opening(
        RoutingKey::parse(IDEA_DISCARDED).expect("a declared routing key is fine"),
        json!({ "nothing": "useful" }),
        clock::at(1_000),
    );

    // then: hearing it fails
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
        clock::at(1_000),
    )
}

#[tokio::test]
async fn a_deleted_scene_is_taken_out_of_the_book() {
    // given: a book holding two scenes
    let wired = a_workbench();
    let (outline, chapter) = a_book_holding(&wired, &["scene_1", "scene_2"]).await;

    // when: the deletion of one is heard
    wired
        .on_scene_deleted
        .handle(&scene_deleted("scene_1"))
        .await
        .expect("taking it out should succeed");
    wired.settle().await;

    // then: the book holds only the other
    let standing = wired
        .outlines
        .read(&outline.to_string(), &eventsourcing::an_author())
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
        .open("project_1", &eventsourcing::an_author())
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
            &eventsourcing::an_author(),
        )
        .await
        .expect("attaching should succeed");

    (outline, chapter)
}

#[tokio::test]
async fn a_scene_deleted_before_its_placement_was_indexed_still_leaves_the_book() {
    // given: a scene placed in a book before the index heard of it
    let wired = a_workbench();
    let (outline, chapter) = placed_but_not_yet_indexed(&wired, a_scene("scene_1")).await;

    // when: its deletion is heard
    wired
        .on_scene_deleted
        .handle(&scene_deleted("scene_1"))
        .await
        .expect("hearing the deletion first should succeed");
    wired.settle().await;

    // then: the book holds nothing
    let standing = wired
        .outlines
        .read(&outline.to_string(), &eventsourcing::an_author())
        .await
        .expect("reading should succeed");
    assert!(
        standing.state.attachments_in(&chapter).is_empty(),
        "the index learns of the placement late, and must let go of what is already gone"
    );
}

#[tokio::test]
async fn an_idea_discarded_before_its_placement_was_indexed_still_leaves_the_book() {
    // given: an idea placed in a book before the index heard of it
    let wired = a_workbench();
    let (outline, chapter) = placed_but_not_yet_indexed(&wired, an_idea("idea_1")).await;

    // when: its discard is heard
    wired
        .on_idea_discarded
        .handle(&discarded("idea_1"))
        .await
        .expect("hearing the discard first should succeed");
    wired.settle().await;

    // then: the book holds nothing
    let standing = wired
        .outlines
        .read(&outline.to_string(), &eventsourcing::an_author())
        .await
        .expect("reading should succeed");
    assert!(
        standing.state.attachments_in(&chapter).is_empty(),
        "the index learns of the placement late, and must let go of what is already gone"
    );
}

#[tokio::test]
async fn hearing_the_same_scene_deletion_twice_is_harmless() {
    // given: a book holding a scene
    let wired = a_workbench();
    a_book_holding(&wired, &["scene_1"]).await;

    // then: hearing its deletion twice succeeds each time
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
    // given: a book holding a scene
    let wired = a_workbench();
    a_book_holding(&wired, &["scene_1"]).await;

    // then: hearing the deletion of a scene no book holds succeeds
    wired
        .on_scene_deleted
        .handle(&scene_deleted("scene_never_placed"))
        .await
        .expect("a scene outside every book is not a failure");
}

#[tokio::test]
async fn a_scene_deletion_that_is_not_shaped_like_one_is_refused() {
    // given: a scene deletion that is no published event
    let wired = a_workbench();
    let nonsense = Message::opening(
        RoutingKey::parse(SCENE_DELETED).expect("a declared routing key is fine"),
        json!({ "nothing": "useful" }),
        clock::at(1_000),
    );

    // then: hearing it fails
    assert!(wired.on_scene_deleted.handle(&nonsense).await.is_err());
}

#[tokio::test]
async fn the_scene_deletion_listener_hears_only_deletions() {
    // expect: scene deletions are heard
    let wired = a_workbench();

    assert!(
        wired
            .on_scene_deleted
            .hears(&RoutingKey::parse(SCENE_DELETED).expect("a plain key is fine"))
    );
    // and: an idea being linked to a scene is not
    assert!(
        !wired
            .on_scene_deleted
            .hears(&RoutingKey::parse("scene.idea.linked").expect("a plain key is fine")),
        "linking an idea leaves a scene where it is in the book"
    );
}

#[tokio::test]
async fn a_tombstone_is_laid_when_the_idea_was_discarded_or_the_scene_deleted() {
    // given: a discarded idea and a deleted scene
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

    // then: their tombstones, the outline's and what they belonged to, were laid when they went, and are forgotten once
    for (before, forgotten) in [
        (clock::at(1_000), 0),
        (clock::at(1_001), 4),
        (clock::at(1_001), 0),
    ] {
        assert_eq!(
            wired
                .outlines
                .forget_tombstones_laid_before(before, u64::MAX)
                .await
                .expect("forgetting tombstones should succeed"),
            forgotten,
            "laid when it happened, and forgotten for good"
        );
    }
}

#[tokio::test]
async fn a_deletion_lays_the_tombstone_and_leaves_detaching_to_a_follow_up() {
    // given: a book holding a scene
    let wired = a_workbench();
    let (outline, chapter) = a_book_holding(&wired, &["scene_1"]).await;

    // when: the scene's deletion is heard
    wired
        .on_scene_deleted
        .handle(&scene_deleted("scene_1"))
        .await
        .expect("the listener should take this message");

    // then: the book still holds it
    assert_eq!(
        wired
            .outlines
            .read(&outline.to_string(), &eventsourcing::an_author())
            .await
            .expect("reading should succeed")
            .state
            .attachments_in(&chapter),
        vec![a_scene("scene_1")],
        "the scene is detached in a hearing of its own, once the tombstone has committed"
    );
    // and: its letting go waits to be published
    assert_eq!(
        wired.outbox.waiting_to_be_published(),
        1,
        "the follow-up waits in the outbox, written in the deletion's transaction"
    );
}

#[tokio::test]
async fn a_deletion_whose_hearing_does_not_commit_lays_no_tombstone_and_asks_nothing() {
    // given: a scene deletion listener whose hearings do not commit
    let wired = a_workbench();
    let listener = Inboxed::new(
        DetachOnSceneDeletedListener::new(
            wired.outlines.clone(),
            wired.outbox.clone(),
            wired.clock.clone(),
        ),
        Arc::new(InMemoryTransactions::refusing_commits()),
        Arc::new(InMemoryInbox::new(wired.clock.clone())),
    );

    // when: a scene's deletion is heard
    listener
        .handle(&scene_deleted("scene_1"))
        .await
        .expect_err("a hearing that does not commit is not handled");

    // then: nothing waits to be published
    assert_eq!(wired.outbox.waiting_to_be_published(), 0);
    // and: no tombstone is laid
    assert_eq!(
        wired
            .outlines
            .forget_tombstones_laid_before(clock::at(1_001), u64::MAX)
            .await
            .expect("forgetting tombstones should succeed"),
        0,
        "the tombstone is laid in the hearing's transaction"
    );
}

#[tokio::test]
async fn a_book_holding_several_gone_scenes_lets_go_of_them_one_hearing_after_another() {
    // given: a book holding three scenes, two of them gone
    let wired = a_workbench();
    let (outline, chapter) = a_book_holding(&wired, &["scene_1", "scene_2", "scene_3"]).await;
    for scene in ["scene_1", "scene_2"] {
        wired
            .outlines
            .lay_tombstone(None, &a_scene(scene), clock::at(1_000))
            .await
            .expect("laying a tombstone should succeed");
    }

    // when: the book is asked to let go of the gone
    wired
        .detaching_the_gone
        .handle(&asked_to_detach_the_gone(&outline))
        .await
        .expect("detaching should succeed");
    // then: it lets go of one
    let held = |wired: &Wired| {
        let outlines = wired.outlines.clone();
        let outline = outline.to_string();
        async move {
            outlines
                .read(&outline, &eventsourcing::an_author())
                .await
                .expect("reading should succeed")
                .state
                .attachments_in(&chapter)
        }
    };
    assert_eq!(
        held(&wired).await.len(),
        2,
        "one scene is detached in a hearing, as two detaches on one book would collide in one transaction"
    );
    // when: what that asks for next is heard
    wired.settle().await;

    // then: it lets go of the other too
    assert_eq!(
        held(&wired).await,
        vec![a_scene("scene_3")],
        "the detach's own event asks the book to let go of the next"
    );
}
