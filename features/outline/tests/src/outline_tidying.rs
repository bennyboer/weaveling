use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{Agent, AgentId};
use messaging::RoutingKey;
use outline_contract::{
    PASSAGE_ATTACHED, PASSAGE_DETACHED, SECTION_ADDED, SECTION_MOVED, SECTION_REMOVED, STARTED,
};
use outline_core::{OutlineCatalog, OutlineId, PassageLink, SectionId, SectionTitle};
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

fn a_passage(named: &str) -> PassageLink {
    PassageLink::from(named)
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

    for changing in [PASSAGE_ATTACHED, PASSAGE_DETACHED, SECTION_REMOVED] {
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
    for quiet in [SECTION_ADDED, SECTION_MOVED, PASSAGE_ATTACHED] {
        assert!(
            !wired
                .projector
                .hears(&RoutingKey::parse(quiet).expect("a declared key is fine")),
            "which outline a project has cannot change when {quiet} happens"
        );
    }
}
