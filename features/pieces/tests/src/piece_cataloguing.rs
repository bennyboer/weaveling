use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{Agent, AggregateId, EventMetadata, Recorded, Version};
use messaging::{Listener, Message, RoutingKey};
use pieces_core::{KIND, PieceCatalog, PieceEvent, PieceId, PieceTitle, ProjectLink};
use pieces_messaging::message_for;
use serde_json::json;
use time::{Duration, OffsetDateTime};

use crate::wiring::{Wired, wired};

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn an_author() -> Agent {
    Agent::Anonymous
}

fn a_workbench() -> Wired {
    wired(Arc::new(FixedClock::new(at(1_000))))
}

async fn listed(wired: &Wired) -> Vec<String> {
    wired
        .catalog
        .in_project(&ProjectLink::from("project_1"))
        .await
        .expect("looking should succeed")
        .into_iter()
        .map(|summary| summary.title.to_string())
        .collect()
}

async fn a_captured_piece(wired: &Wired) -> PieceId {
    wired
        .pieces
        .capture("project_1", "The Loom", &an_author())
        .await
        .expect("capturing should succeed")
}

fn a_title() -> PieceTitle {
    PieceTitle::new("The Loom").expect("a plain title is fine")
}

fn message_about(id: &PieceId, event: PieceEvent) -> Message {
    let happened = Recorded {
        metadata: EventMetadata {
            aggregate: AggregateId::from(id),
            kind: KIND,
            version: Version::of(1),
            agent: an_author(),
            occurred_at: at(1_000),
            is_snapshot: false,
        },
        event,
    };

    message_for(&happened).expect("this event should be published")
}

fn a_captured_piece_told_of(id: &PieceId) -> Message {
    message_about(
        id,
        PieceEvent::Captured {
            project: ProjectLink::from("project_1"),
            title: a_title(),
        },
    )
}

#[tokio::test]
async fn a_captured_piece_reaches_the_listing() {
    let wired = a_workbench();

    a_captured_piece(&wired).await;

    assert_eq!(listed(&wired).await, ["The Loom"]);
}

#[tokio::test]
async fn hearing_the_same_message_twice_leaves_one_piece() {
    let wired = a_workbench();
    let id = a_captured_piece(&wired).await;

    wired
        .cataloguing
        .handle(&a_captured_piece_told_of(&id))
        .await
        .expect("a redelivery is not a failure");

    assert_eq!(
        listed(&wired).await,
        ["The Loom"],
        "a broker redelivers, so hearing twice must count once"
    );
}

#[tokio::test]
async fn a_stale_message_cannot_resurrect_a_discarded_piece() {
    let wired = a_workbench();
    let id = a_captured_piece(&wired).await;
    wired
        .pieces
        .discard(&id.to_string(), None, &an_author())
        .await
        .expect("discarding should succeed");
    assert!(listed(&wired).await.is_empty(), "the discard was projected");

    wired
        .cataloguing
        .handle(&a_captured_piece_told_of(&id))
        .await
        .expect("hearing a stale message is not a failure");

    assert!(
        listed(&wired).await.is_empty(),
        "the projector reads the piece rather than trusting the message, so order cannot bite"
    );
}

#[tokio::test]
async fn the_listing_follows_the_latest_title_however_messages_arrive() {
    let wired = a_workbench();
    let id = a_captured_piece(&wired).await;
    wired
        .pieces
        .retitle(&id.to_string(), "The Silent Loom", None, &an_author())
        .await
        .expect("retitling should succeed");

    wired
        .cataloguing
        .handle(&a_captured_piece_told_of(&id))
        .await
        .expect("hearing a stale message is not a failure");

    assert_eq!(
        listed(&wired).await,
        ["The Silent Loom"],
        "an out of order redelivery must not roll the listing back"
    );
}

#[tokio::test]
async fn a_message_about_no_piece_at_all_is_refused() {
    let wired = a_workbench();
    let nonsense = Message::opening(
        RoutingKey::parse("piece.captured").expect("a plain key is fine"),
        json!({ "nothing": "useful" }),
        at(1_000),
    );

    let refused = wired.cataloguing.handle(&nonsense).await;

    assert!(
        refused.is_err(),
        "a message the projector cannot act on belongs in dead letters, not silently dropped"
    );
}

#[tokio::test]
async fn a_message_about_a_piece_that_was_never_stored_is_refused() {
    let wired = a_workbench();
    let never_stored = PieceId::generate(at(1_000));

    let refused = wired
        .cataloguing
        .handle(&a_captured_piece_told_of(&never_stored))
        .await;

    assert!(
        refused.is_err(),
        "the event is appended before it is published, so this can only mean something is wrong"
    );
}
