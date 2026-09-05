use std::sync::Arc;

use eventsourcing::{Agent, AgentId};

use boards_contract::{PIECE_MOVED, PIECE_PINNED, PIECE_UNPINNED, STARTED};
use boards_core::{BoardCatalog, BoardId, PieceLink, Size, Spot};
use clock::FixedClock;
use messaging::{Message, RoutingKey};
use pieces_contract::{DISCARDED, PieceEventDTO};
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

fn a_piece(named: &str) -> PieceLink {
    PieceLink::from(named)
}

fn discarded(piece: &str) -> Message {
    Message::opening(
        RoutingKey::parse(DISCARDED).expect("a declared routing key is fine"),
        json!({
            "event": { "version": 0, "name": "DISCARDED" },
            "aggregate": { "id": piece, "kind": "piece", "version": 4 },
            "agent": { "kind": "anonymous", "id": null },
            "occurred_at": "1970-01-01T00:16:40Z",
        }),
        at(1_000),
    )
}

async fn a_board_holding(wired: &Wired, pieces: &[&str]) -> BoardId {
    let id = wired
        .boards
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed")
        .id;

    for (nth, piece) in pieces.iter().enumerate() {
        wired
            .boards
            .pin(
                &id.to_string(),
                a_piece(piece),
                Spot::at(nth as i64 * 10, 0),
                Size::CARD,
                None,
                &an_author(),
            )
            .await
            .expect("pinning should succeed");
    }

    id
}

async fn pinned_on(wired: &Wired, board: &BoardId) -> Vec<String> {
    wired
        .boards
        .get(&board.to_string())
        .await
        .expect("reading should succeed")
        .state
        .pieces()
        .into_iter()
        .map(|positioned| positioned.piece.to_string())
        .collect()
}

#[tokio::test]
async fn pinning_a_piece_puts_it_in_the_index() {
    let wired = a_workbench();

    let board = a_board_holding(&wired, &["piece_1"]).await;

    assert_eq!(
        wired
            .catalog
            .boards_holding(&a_piece("piece_1"))
            .await
            .expect("looking should succeed"),
        vec![board],
        "the board answers which of its own pieces it holds, from its own events"
    );
}

#[tokio::test]
async fn unpinning_a_piece_takes_it_out_of_the_index() {
    let wired = a_workbench();
    let board = a_board_holding(&wired, &["piece_1"]).await;

    wired
        .boards
        .unpin(&board.to_string(), a_piece("piece_1"), None, &an_author())
        .await
        .expect("unpinning should succeed");

    assert!(
        wired
            .catalog
            .boards_holding(&a_piece("piece_1"))
            .await
            .expect("looking should succeed")
            .is_empty()
    );
}

#[tokio::test]
async fn a_discarded_piece_is_taken_off_the_board() {
    let wired = a_workbench();
    let board = a_board_holding(&wired, &["piece_1", "piece_2"]).await;

    wired
        .tidier
        .handle(&discarded("piece_1"))
        .await
        .expect("tidying should succeed");

    assert_eq!(
        pinned_on(&wired, &board).await,
        ["piece_2"],
        "a discarded piece leaves, and the others stay where they were"
    );
}

#[tokio::test]
async fn hearing_the_same_discard_twice_is_harmless() {
    let wired = a_workbench();
    let board = a_board_holding(&wired, &["piece_1"]).await;

    for _ in 0..2 {
        wired
            .tidier
            .handle(&discarded("piece_1"))
            .await
            .expect("a redelivery is not a failure");
    }

    assert!(
        pinned_on(&wired, &board).await.is_empty(),
        "a broker redelivers, so the second unpin must find nothing left to do"
    );
}

#[tokio::test]
async fn discarding_a_piece_nobody_pinned_is_harmless() {
    let wired = a_workbench();
    a_board_holding(&wired, &["piece_1"]).await;

    wired
        .tidier
        .handle(&discarded("piece_never_pinned"))
        .await
        .expect("a piece that was never on a board is not a failure");
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
async fn the_index_hears_pinning_and_unpinning_and_nothing_else() {
    let wired = a_workbench();

    for pinning in [PIECE_PINNED, PIECE_UNPINNED] {
        assert!(
            wired
                .indexer
                .hears(&RoutingKey::parse(pinning).expect("a declared key is fine")),
            "{pinning} changes what a board holds, so the index must hear it"
        );
    }
    for quiet in [STARTED, PIECE_MOVED] {
        assert!(
            !wired
                .indexer
                .hears(&RoutingKey::parse(quiet).expect("a declared key is fine")),
            "{quiet} leaves the set of pieces alone, so it must not cost a projection write"
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
            .hears(&RoutingKey::parse("piece.retitled").expect("a plain key is fine")),
        "nothing else a piece does should move it off a board"
    );
}

#[tokio::test]
async fn what_the_discard_message_says_matches_the_published_shape() {
    let read: PieceEventDTO = serde_json::from_value(discarded("piece_1").payload["event"].clone())
        .expect("the fixture should parse as a published piece event");

    assert_eq!(
        read,
        PieceEventDTO::Discarded,
        "if this stops parsing, the pieces contract moved and this listener is deaf"
    );
}
