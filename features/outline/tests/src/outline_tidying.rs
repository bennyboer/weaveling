use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{Agent, AgentId};
use ideas_contract::{DISCARDED, IdeaEventDTO};
use messaging::{Message, RoutingKey};
use outline_contract::{
    IDEA_ATTACHED, IDEA_DETACHED, SECTION_ADDED, SECTION_MOVED, SECTION_REMOVED, STARTED,
};
use outline_core::{IdeaLink, OutlineCatalog, OutlineId, SectionId, SectionTitle};
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

fn a_idea(named: &str) -> IdeaLink {
    IdeaLink::from(named)
}

fn titled(what: &str) -> SectionTitle {
    SectionTitle::new(what).expect("the title should be usable")
}

fn discarded(idea: &str) -> Message {
    Message::opening(
        RoutingKey::parse(DISCARDED).expect("a declared routing key is fine"),
        json!({
            "event": { "version": 0, "name": "DISCARDED" },
            "aggregate": { "id": idea, "kind": "idea", "version": 4 },
            "agent": { "kind": "anonymous", "id": null },
            "occurred_at": "1970-01-01T00:16:40Z",
        }),
        at(1_000),
    )
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

async fn a_book_holding(wired: &Wired, ideas: &[&str]) -> (OutlineId, SectionId) {
    let outline = wired
        .outlines
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed")
        .id;
    let chapter = a_chapter(wired, &outline, "Chapter 1").await;

    let mut behind = None;
    for idea in ideas {
        wired
            .outlines
            .attach(
                &outline.to_string(),
                a_idea(idea),
                chapter,
                behind.clone(),
                None,
                &an_author(),
            )
            .await
            .expect("attaching should succeed");
        behind = Some(a_idea(idea));
    }

    wired.settle().await;

    (outline, chapter)
}

async fn reading_order(wired: &Wired, outline: &OutlineId) -> Vec<String> {
    wired
        .outlines
        .get(&outline.to_string())
        .await
        .expect("reading should succeed")
        .state
        .reading_order()
        .into_iter()
        .map(|idea| idea.to_string())
        .collect()
}

#[tokio::test]
async fn attaching_a_idea_puts_it_in_the_index() {
    let wired = a_workbench();

    let (outline, _) = a_book_holding(&wired, &["idea_1"]).await;

    assert_eq!(
        wired
            .catalog
            .outlines_holding(&a_idea("idea_1"))
            .await
            .expect("looking should succeed"),
        vec![outline]
    );
}

#[tokio::test]
async fn detaching_a_idea_takes_it_out_of_the_index() {
    let wired = a_workbench();
    let (outline, _) = a_book_holding(&wired, &["idea_1"]).await;

    wired
        .outlines
        .detach(&outline.to_string(), a_idea("idea_1"), None, &an_author())
        .await
        .expect("detaching should succeed");
    wired.settle().await;

    assert!(
        wired
            .catalog
            .outlines_holding(&a_idea("idea_1"))
            .await
            .expect("looking should succeed")
            .is_empty()
    );
}

#[tokio::test]
async fn removing_a_section_takes_its_ideas_out_of_the_index() {
    let wired = a_workbench();
    let (outline, chapter) = a_book_holding(&wired, &["idea_1"]).await;

    wired
        .outlines
        .remove(&outline.to_string(), chapter, None, &an_author())
        .await
        .expect("removing should succeed");
    wired.settle().await;

    assert!(
        wired
            .catalog
            .outlines_holding(&a_idea("idea_1"))
            .await
            .expect("looking should succeed")
            .is_empty(),
        "a removed section returns its ideas to the pool, and the index has to hear about it"
    );
}

#[tokio::test]
async fn a_discarded_idea_leaves_the_book() {
    let wired = a_workbench();
    let (outline, _) = a_book_holding(&wired, &["idea_1", "idea_2"]).await;

    wired
        .tidier
        .handle(&discarded("idea_1"))
        .await
        .expect("tidying should succeed");
    wired.settle().await;

    assert_eq!(
        reading_order(&wired, &outline).await,
        ["idea_2"],
        "a discarded idea leaves, and the rest of the chapter reads on"
    );
}

#[tokio::test]
async fn hearing_the_same_discard_twice_is_harmless() {
    let wired = a_workbench();
    let (outline, _) = a_book_holding(&wired, &["idea_1"]).await;

    for _ in 0..2 {
        wired
            .tidier
            .handle(&discarded("idea_1"))
            .await
            .expect("a redelivery is not a failure");
    }
    wired.settle().await;

    assert!(
        reading_order(&wired, &outline).await.is_empty(),
        "a broker redelivers before the index it reads has caught up, so the second detach finds \
         the idea still listed and must treat an already-detached idea as done"
    );
}

#[tokio::test]
async fn discarding_a_idea_that_was_never_in_the_book_is_harmless() {
    let wired = a_workbench();
    a_book_holding(&wired, &["idea_1"]).await;

    wired
        .tidier
        .handle(&discarded("idea_never_placed"))
        .await
        .expect("a idea that was only ever in the pool is not a failure");
}

#[tokio::test]
async fn a_discard_that_is_not_a_published_event_is_refused() {
    let wired = a_workbench();
    let nonsense = Message::opening(
        RoutingKey::parse(DISCARDED).expect("a declared routing key is fine"),
        json!({ "nothing": "useful" }),
        at(1_000),
    );

    assert!(
        wired.tidier.handle(&nonsense).await.is_err(),
        "a message the listener cannot act on belongs in dead letters"
    );
}

#[tokio::test]
async fn the_index_hears_everything_that_changes_what_the_book_holds() {
    let wired = a_workbench();

    for changing in [IDEA_ATTACHED, IDEA_DETACHED, SECTION_REMOVED] {
        assert!(
            wired
                .indexer
                .hears(&RoutingKey::parse(changing).expect("a declared key is fine")),
            "{changing} changes which ideas are in the book, so the index must hear it"
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
    for quiet in [SECTION_ADDED, SECTION_MOVED, IDEA_ATTACHED] {
        assert!(
            !wired
                .projector
                .hears(&RoutingKey::parse(quiet).expect("a declared key is fine")),
            "which outline a project has cannot change when {quiet} happens"
        );
    }
}

#[tokio::test]
async fn the_discard_listener_hears_only_discards() {
    let wired = a_workbench();

    assert!(
        wired
            .tidier
            .hears(&RoutingKey::parse(DISCARDED).expect("a plain key is fine"))
    );
    assert!(
        !wired
            .tidier
            .hears(&RoutingKey::parse("idea.retitled").expect("a plain key is fine")),
        "nothing else a idea does should take it out of the book"
    );
}

#[tokio::test]
async fn what_the_discard_message_says_matches_the_published_shape() {
    let read: IdeaEventDTO = serde_json::from_value(discarded("idea_1").payload["event"].clone())
        .expect("the fixture should parse as a published idea event");

    assert_eq!(
        read,
        IdeaEventDTO::Discarded,
        "if this stops parsing, the ideas contract moved and this listener is deaf"
    );
}
