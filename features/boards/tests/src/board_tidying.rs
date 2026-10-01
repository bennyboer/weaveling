use std::sync::Arc;

use eventsourcing::{Agent, AgentId};

use boards_contract::{IDEA_MOVED, IDEA_PINNED, IDEA_UNPINNED, STARTED};
use boards_core::{BoardCatalog, BoardId, IdeaLink, Size, Spot};
use clock::FixedClock;
use ideas_contract::{DISCARDED, IdeaEventDTO};
use messaging::{Message, RoutingKey};
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

fn an_idea(named: &str) -> IdeaLink {
    IdeaLink::from(named)
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

async fn a_board_holding(wired: &Wired, ideas: &[&str]) -> BoardId {
    let id = wired
        .boards
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed")
        .id;

    for (nth, idea) in ideas.iter().enumerate() {
        wired
            .boards
            .pin(
                &id.to_string(),
                an_idea(idea),
                Spot::at(nth as i64 * 10, 0),
                Size::CARD,
                None,
                &an_author(),
            )
            .await
            .expect("pinning should succeed");
    }

    wired.settle().await;

    id
}

async fn pinned_on(wired: &Wired, board: &BoardId) -> Vec<String> {
    wired
        .boards
        .get(&board.to_string())
        .await
        .expect("reading should succeed")
        .state
        .ideas()
        .into_iter()
        .map(|positioned| positioned.idea.to_string())
        .collect()
}

#[tokio::test]
async fn pinning_an_idea_puts_it_in_the_index() {
    let wired = a_workbench();

    let board = a_board_holding(&wired, &["idea_1"]).await;

    assert_eq!(
        wired
            .catalog
            .boards_holding(&an_idea("idea_1"))
            .await
            .expect("looking should succeed"),
        vec![board],
        "the board answers which of its own ideas it holds, from its own events"
    );
}

#[tokio::test]
async fn unpinning_an_idea_takes_it_out_of_the_index() {
    let wired = a_workbench();
    let board = a_board_holding(&wired, &["idea_1"]).await;

    wired
        .boards
        .unpin(&board.to_string(), an_idea("idea_1"), None, &an_author())
        .await
        .expect("unpinning should succeed");
    wired.settle().await;

    assert!(
        wired
            .catalog
            .boards_holding(&an_idea("idea_1"))
            .await
            .expect("looking should succeed")
            .is_empty()
    );
}

#[tokio::test]
async fn a_discarded_idea_is_taken_off_the_board() {
    let wired = a_workbench();
    let board = a_board_holding(&wired, &["idea_1", "idea_2"]).await;

    wired
        .tidier
        .handle(&discarded("idea_1"))
        .await
        .expect("tidying should succeed");
    wired.settle().await;

    assert_eq!(
        pinned_on(&wired, &board).await,
        ["idea_2"],
        "a discarded idea leaves, and the others stay where they were"
    );
}

#[tokio::test]
async fn hearing_the_same_discard_twice_is_harmless() {
    let wired = a_workbench();
    let board = a_board_holding(&wired, &["idea_1"]).await;

    for _ in 0..2 {
        wired
            .tidier
            .handle(&discarded("idea_1"))
            .await
            .expect("a redelivery is not a failure");
    }
    wired.settle().await;

    assert!(
        pinned_on(&wired, &board).await.is_empty(),
        "a broker redelivers, so the second unpin must find nothing left to do"
    );
}

#[tokio::test]
async fn discarding_an_idea_nobody_pinned_is_harmless() {
    let wired = a_workbench();
    a_board_holding(&wired, &["idea_1"]).await;

    wired
        .tidier
        .handle(&discarded("idea_never_pinned"))
        .await
        .expect("an idea that was never on a board is not a failure");
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

    for pinning in [IDEA_PINNED, IDEA_UNPINNED] {
        assert!(
            wired
                .indexer
                .hears(&RoutingKey::parse(pinning).expect("a declared key is fine")),
            "{pinning} changes what a board holds, so the index must hear it"
        );
    }
    for quiet in [STARTED, IDEA_MOVED] {
        assert!(
            !wired
                .indexer
                .hears(&RoutingKey::parse(quiet).expect("a declared key is fine")),
            "{quiet} leaves the set of ideas alone, so it must not cost a projection write"
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
        "nothing else an idea does should move it off a board"
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
