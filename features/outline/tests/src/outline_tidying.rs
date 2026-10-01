use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{Agent, AgentId};
use ideas_contract::DISCARDED as IDEA_DISCARDED;
use messaging::{Message, RoutingKey};
use outline_contract::{
    ATTACHED, DETACHED, SECTION_ADDED, SECTION_MOVED, SECTION_REMOVED, STARTED,
};
use outline_core::{Attachment, OutlineCatalog, OutlineId, SectionId, SectionTitle};
use serde_json::json;
use time::{Duration, OffsetDateTime};

use crate::wiring::{Wired, wired};

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn an_author() -> Agent {
    Agent::User(AgentId::from("author-7"))
}

fn a_workbench() -> Wired {
    wired(Arc::new(FixedClock::new(at(1_000))))
}

fn a_passage(named: &str) -> Attachment {
    Attachment::passage(named)
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

async fn a_book_holding(wired: &Wired, passages: &[&str]) -> (OutlineId, SectionId) {
    let outline = wired
        .outlines
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed")
        .id;
    let chapter = a_chapter(wired, &outline, "Chapter 1").await;

    let mut behind = None;
    for passage in passages {
        wired
            .outlines
            .attach(
                &outline.to_string(),
                a_passage(passage),
                chapter,
                behind.clone(),
                None,
                &an_author(),
            )
            .await
            .expect("attaching should succeed");
        behind = Some(a_passage(passage));
    }

    wired.settle().await;

    (outline, chapter)
}

#[tokio::test]
async fn attaching_a_passage_puts_it_in_the_index() {
    let wired = a_workbench();

    let (outline, _) = a_book_holding(&wired, &["passage_1"]).await;

    assert_eq!(
        wired
            .catalog
            .outlines_holding(&a_passage("passage_1"))
            .await
            .expect("looking should succeed"),
        vec![outline]
    );
}

#[tokio::test]
async fn detaching_a_passage_takes_it_out_of_the_index() {
    let wired = a_workbench();
    let (outline, _) = a_book_holding(&wired, &["passage_1"]).await;

    wired
        .outlines
        .detach(
            &outline.to_string(),
            a_passage("passage_1"),
            None,
            &an_author(),
        )
        .await
        .expect("detaching should succeed");
    wired.settle().await;

    assert!(
        wired
            .catalog
            .outlines_holding(&a_passage("passage_1"))
            .await
            .expect("looking should succeed")
            .is_empty()
    );
}

#[tokio::test]
async fn removing_a_section_takes_its_passages_out_of_the_index() {
    let wired = a_workbench();
    let (outline, chapter) = a_book_holding(&wired, &["passage_1"]).await;

    wired
        .outlines
        .remove(&outline.to_string(), chapter, None, &an_author())
        .await
        .expect("removing should succeed");
    wired.settle().await;

    assert!(
        wired
            .catalog
            .outlines_holding(&a_passage("passage_1"))
            .await
            .expect("looking should succeed")
            .is_empty(),
        "a removed section returns its passages to the pool, and the index has to hear about it"
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
            "{changing} changes which passages are in the book, so the index must hear it"
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
    let (outline, chapter) = a_book_holding(wired, &["passage_1"]).await;

    wired
        .outlines
        .attach(
            &outline.to_string(),
            an_idea(idea),
            chapter,
            Some(a_passage("passage_1")),
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
        .tidier
        .handle(&discarded("idea_1"))
        .await
        .expect("tidying should succeed");

    let standing = wired
        .outlines
        .get(&outline.to_string())
        .await
        .expect("reading should succeed");
    assert_eq!(
        standing.state.attachments_in(&chapter),
        vec![a_passage("passage_1")],
        "a note about an idea nobody kept is a reference to nothing"
    );
}

#[tokio::test]
async fn discarding_an_idea_leaves_the_prose_alone() {
    let wired = a_workbench();
    let (outline, chapter) = a_book_noting(&wired, "idea_1").await;

    wired
        .tidier
        .handle(&discarded("idea_1"))
        .await
        .expect("tidying should succeed");

    let standing = wired
        .outlines
        .get(&outline.to_string())
        .await
        .expect("reading should succeed");
    assert_eq!(
        standing.state.reading_order().len(),
        1,
        "ideas are disposable and passages are the book, so throwing one away must never \
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
            .tidier
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
        .tidier
        .handle(&discarded("idea_elsewhere"))
        .await
        .expect("an idea in no book is nothing to do");
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
        wired.tidier.handle(&nonsense).await.is_err(),
        "a message the listener cannot act on belongs in dead letters, not silently dropped"
    );
}
