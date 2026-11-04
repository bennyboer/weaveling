use std::sync::Arc;
use std::sync::atomic::Ordering;

use eventsourcing::{AccountId, Agent, AggregateId, EventStore, Version};

use boards_contract::{IDEA_MOVED, IDEA_PINNED, IDEA_UNPINNED, STARTED};
use boards_core::{BoardCatalog, BoardEvent, BoardId, IdeaLink, KIND, Size, Spot};
use boards_messaging::{
    LET_GO_EVERYWHERE, LET_GO_OF_THE_GONE, LetGoEverywhereDTO, LetGoOfTheGoneDTO,
    UnpinOnDiscardListener,
};
use clock::FixedClock;
use ideas_contract::{DISCARDED, IdeaEventDTO};
use inbox::{InMemoryInbox, Inboxed};
use messaging::{Listener, Message, RoutingKey};
use serde_json::json;
use time::{Duration, OffsetDateTime};
use transacting::{InMemoryTransactions, Transactions};

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

fn an_idea(named: &str) -> IdeaLink {
    IdeaLink::from(named)
}

fn discarded(idea: &str) -> Message {
    Message::opening(
        RoutingKey::parse(DISCARDED).expect("a declared routing key is fine"),
        json!({
            "event": { "version": 0, "name": "DISCARDED" },
            "aggregate": { "id": idea, "kind": "idea", "version": 4 },
            "agent": { "kind": "local", "id": null },
            "occurred_at": "1970-01-01T00:16:40Z",
        }),
        at(1_000),
    )
}

fn asked_to_let_go_everywhere(idea: &str) -> Message {
    Message::opening(
        RoutingKey::parse(LET_GO_EVERYWHERE).expect("a follow-up's key holds no wildcards"),
        serde_json::to_value(LetGoEverywhereDTO {
            idea: idea.to_owned(),
        })
        .expect("a follow-up is plain data"),
        at(1_000),
    )
}

fn asked_to_let_go_of_the_gone(board: &BoardId) -> Message {
    Message::opening(
        RoutingKey::parse(LET_GO_OF_THE_GONE).expect("a follow-up's key holds no wildcards"),
        serde_json::to_value(LetGoOfTheGoneDTO {
            board: board.to_string(),
        })
        .expect("a follow-up is plain data"),
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
        .read(&board.to_string(), &an_author())
        .await
        .expect("reading should succeed")
        .state
        .ideas()
        .into_iter()
        .map(|positioned| positioned.idea.to_string())
        .collect()
}

#[tokio::test]
async fn an_idea_discarded_before_its_pin_was_indexed_still_leaves_the_board() {
    // given: an idea pinned to a board
    let wired = a_workbench();
    let board = wired
        .boards
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed")
        .id;
    wired
        .boards
        .pin(
            &board.to_string(),
            an_idea("idea_1"),
            Spot::at(0, 0),
            Size::CARD,
            None,
            &an_author(),
        )
        .await
        .expect("pinning should succeed");

    // when: the idea's discard is heard before the pin is indexed
    wired
        .on_idea_discarded
        .handle(&discarded("idea_1"))
        .await
        .expect("hearing the discard first should succeed");
    wired.settle().await;

    // then: the idea leaves the board
    assert!(
        pinned_on(&wired, &board).await.is_empty(),
        "the index learns of the pin late, and must let go of what is already gone"
    );
}

#[tokio::test]
async fn pinning_an_idea_puts_it_in_the_index() {
    // given: an idea
    let wired = a_workbench();

    // when: a board holds it
    let board = a_board_holding(&wired, &["idea_1"]).await;

    // then: the index says the board holds it
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
    // given: a board holding an idea
    let wired = a_workbench();
    let board = a_board_holding(&wired, &["idea_1"]).await;

    // when: the idea is unpinned
    wired
        .boards
        .unpin(&board.to_string(), an_idea("idea_1"), None, &an_author())
        .await
        .expect("unpinning should succeed");
    wired.settle().await;

    // then: the index no longer says any board holds it
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
    // given: a board holding two ideas
    let wired = a_workbench();
    let board = a_board_holding(&wired, &["idea_1", "idea_2"]).await;

    // when: one of them is discarded
    wired
        .on_idea_discarded
        .handle(&discarded("idea_1"))
        .await
        .expect("tidying should succeed");
    wired.settle().await;

    // then: only the other stays on the board
    assert_eq!(
        pinned_on(&wired, &board).await,
        ["idea_2"],
        "a discarded idea leaves, and the others stay where they were"
    );
}

#[tokio::test]
async fn hearing_the_same_discard_twice_is_harmless() {
    // given: a board holding an idea
    let wired = a_workbench();
    let board = a_board_holding(&wired, &["idea_1"]).await;

    // when: its discard is heard twice
    for _ in 0..2 {
        wired
            .on_idea_discarded
            .handle(&discarded("idea_1"))
            .await
            .expect("a redelivery is not a failure");
    }
    wired.settle().await;

    // then: it is off the board
    assert!(
        pinned_on(&wired, &board).await.is_empty(),
        "a broker redelivers, so the second unpin must find nothing left to do"
    );
}

#[tokio::test]
async fn discarding_an_idea_nobody_pinned_is_harmless() {
    // given: a board holding an idea
    let wired = a_workbench();
    a_board_holding(&wired, &["idea_1"]).await;

    // then: hearing of the discard of an idea nobody pinned succeeds
    wired
        .on_idea_discarded
        .handle(&discarded("idea_never_pinned"))
        .await
        .expect("an idea that was never on a board is not a failure");
}

async fn pinned_as_published(wired: &Wired, board: &BoardId) -> Message {
    let recorded = wired
        .store
        .read_from(&AggregateId::from(board), KIND, Version::ZERO)
        .await
        .expect("reading should succeed")
        .into_iter()
        .find(|recorded| matches!(recorded.event, BoardEvent::IdeaPinned { .. }))
        .expect("the board was pinned to");

    boards_messaging::message_for(&recorded).expect("a pin is published")
}

#[tokio::test]
async fn a_discarded_idea_is_let_go_of_in_the_system_s_name() {
    // given: a board holding an idea
    let wired = a_workbench();
    let board = a_board_holding(&wired, &["idea_1"]).await;

    // when: the idea is discarded
    wired
        .on_idea_discarded
        .handle(&discarded("idea_1"))
        .await
        .expect("tidying should succeed");
    wired.settle().await;

    // then: it is let go of in the system's name
    let recorded = wired
        .store
        .read_from(&AggregateId::from(&board), KIND, Version::ZERO)
        .await
        .expect("reading should succeed");
    assert_eq!(
        recorded
            .last()
            .expect("the unpin is recorded")
            .metadata
            .agent,
        Agent::System
    );
}

#[tokio::test]
async fn whom_to_let_in_not_being_known_fails_letting_go_so_it_is_tried_again() {
    // given: a board holding an idea
    let authorizer = Arc::new(TheSystemAndTheAuthor::default());
    let wired = a_workbench_guarded_by(authorizer.clone());
    a_board_holding(&wired, &["idea_1"]).await;
    // and: who may do what cannot be known
    authorizer.down.store(true, Ordering::Release);

    // then: letting go of the idea everywhere fails
    assert!(
        wired
            .letting_go_everywhere
            .handle(&asked_to_let_go_everywhere("idea_1"))
            .await
            .is_err(),
        "a discarded idea left pinned for good is worse than a message tried again"
    );
}

#[tokio::test]
async fn a_late_pin_does_not_bring_back_the_ideas_of_a_discarded_board() {
    // given: a discarded board that held an idea
    let wired = a_workbench();
    let board = a_board_holding(&wired, &["idea_1"]).await;
    let late = pinned_as_published(&wired, &board).await;
    wired
        .boards
        .sweep("project_1")
        .await
        .expect("sweeping should succeed");
    wired.settle().await;

    // when: the pin of that idea is heard late
    wired
        .indexer
        .handle(&late)
        .await
        .expect("a pin tried again after the discard is not a failure");

    // then: the index does not say any board holds it
    assert!(
        wired
            .catalog
            .boards_holding(&an_idea("idea_1"))
            .await
            .expect("looking should succeed")
            .is_empty(),
        "a discarded board still lists its ideas, so reading it back must not put them in the index again"
    );
}

#[tokio::test]
async fn discarding_an_idea_still_indexed_on_a_discarded_board_is_harmless() {
    // given: a discarded board the index still says holds an idea
    let wired = a_workbench();
    let board = a_board_holding(&wired, &["idea_1"]).await;
    wired
        .boards
        .sweep("project_1")
        .await
        .expect("sweeping should succeed");
    wired.settle().await;
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    wired
        .catalog
        .holds(&mut transaction, board, &[an_idea("idea_1")])
        .await
        .expect("holding should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    // then: hearing of the idea's discard succeeds
    wired
        .on_idea_discarded
        .handle(&discarded("idea_1"))
        .await
        .expect("laying the tombstone should succeed");
    // and: letting go of it everywhere succeeds
    wired
        .letting_go_everywhere
        .handle(&asked_to_let_go_everywhere("idea_1"))
        .await
        .expect(
            "a discarded board has nothing left to unpin, so this must not end in dead letters",
        );
}

#[tokio::test]
async fn a_discard_that_is_not_a_published_event_is_refused() {
    // given: a discard that is not a published event
    let wired = a_workbench();
    let nonsense = Message::opening(
        RoutingKey::parse(DISCARDED).expect("a declared routing key is fine"),
        json!({ "nothing": "useful" }),
        at(1_000),
    );

    // then: hearing it is refused
    assert!(
        wired.on_idea_discarded.handle(&nonsense).await.is_err(),
        "a message the listener cannot act on belongs in dead letters"
    );
}

#[tokio::test]
async fn the_index_hears_pinning_and_unpinning_and_nothing_else() {
    // given: the index
    let wired = a_workbench();

    // then: it hears pinning and unpinning
    for pinning in [IDEA_PINNED, IDEA_UNPINNED] {
        assert!(
            wired
                .indexer
                .hears(&RoutingKey::parse(pinning).expect("a declared key is fine")),
            "{pinning} changes what a board holds, so the index must hear it"
        );
    }
    // and: nothing else
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
    // given: the discard listener
    let wired = a_workbench();

    // then: it hears discards
    assert!(
        wired
            .on_idea_discarded
            .hears(&RoutingKey::parse(DISCARDED).expect("a plain key is fine"))
    );
    // and: nothing else an idea does
    assert!(
        !wired
            .on_idea_discarded
            .hears(&RoutingKey::parse("idea.retitled").expect("a plain key is fine")),
        "nothing else an idea does should move it off a board"
    );
}

#[tokio::test]
async fn what_the_discard_message_says_matches_the_published_shape() {
    // when: the discard message is read as a published idea event
    let read: IdeaEventDTO = serde_json::from_value(discarded("idea_1").payload["event"].clone())
        .expect("the fixture should parse as a published idea event");

    // then: it is a discard
    assert_eq!(
        read,
        IdeaEventDTO::Discarded,
        "if this stops parsing, the ideas contract moved and this listener is deaf"
    );
}

#[tokio::test]
async fn a_discarded_idea_s_tombstone_is_laid_when_it_was_discarded() {
    // given: an idea's discard heard
    let wired = a_workbench();
    wired
        .on_idea_discarded
        .handle(&discarded("idea_1"))
        .await
        .expect("the listener should take this message");

    // then: its tombstone is laid as of the discard, and forgotten once
    for (before, forgotten) in [(at(1_000), 0), (at(1_001), 1), (at(1_001), 0)] {
        assert_eq!(
            wired
                .boards
                .forget_tombstones_laid_before(before, u64::MAX)
                .await
                .expect("forgetting tombstones should succeed"),
            forgotten,
            "laid when it happened, and forgotten for good"
        );
    }
}

#[tokio::test]
async fn a_discard_lays_the_tombstone_and_leaves_letting_go_to_a_follow_up() {
    // given: a board holding an idea
    let wired = a_workbench();
    let board = a_board_holding(&wired, &["idea_1"]).await;

    // when: the idea's discard is heard
    wired
        .on_idea_discarded
        .handle(&discarded("idea_1"))
        .await
        .expect("the listener should take this message");

    // then: the idea is still on the board
    assert_eq!(
        pinned_on(&wired, &board).await,
        ["idea_1"],
        "the idea is let go of in a hearing of its own, once the tombstone has committed"
    );
    // and: letting go of it is asked for
    assert_eq!(
        wired.outbox.waiting_to_be_published(),
        1,
        "the follow-up waits in the outbox, written in the discard's transaction"
    );
}

#[tokio::test]
async fn a_discard_whose_hearing_does_not_commit_lays_no_tombstone_and_asks_nothing() {
    // given: a hearing that does not commit
    let wired = a_workbench();
    let listener = Inboxed::new(
        UnpinOnDiscardListener::new(
            wired.boards.clone(),
            wired.outbox.clone(),
            wired.clock.clone(),
        ),
        Arc::new(InMemoryTransactions::refusing_commits()),
        Arc::new(InMemoryInbox::new(wired.clock.clone())),
    );

    // when: an idea's discard is heard in it
    listener
        .handle(&discarded("idea_1"))
        .await
        .expect_err("a hearing that does not commit is not handled");

    // then: nothing is asked for
    assert_eq!(wired.outbox.waiting_to_be_published(), 0);
    // and: no tombstone is laid
    assert_eq!(
        wired
            .boards
            .forget_tombstones_laid_before(at(1_001), u64::MAX)
            .await
            .expect("forgetting tombstones should succeed"),
        0,
        "the tombstone is laid in the hearing's transaction"
    );
}

#[tokio::test]
async fn a_board_holding_several_gone_ideas_lets_go_of_them_one_hearing_after_another() {
    // given: a board holding three ideas
    let wired = a_workbench();
    let board = a_board_holding(&wired, &["idea_1", "idea_2", "idea_3"]).await;
    // and: two of them gone
    for idea in ["idea_1", "idea_2"] {
        wired
            .boards
            .lay_tombstone(None, idea, at(1_000))
            .await
            .expect("laying a tombstone should succeed");
    }

    // when: the board is asked to let go of the gone
    wired
        .letting_go_of_the_gone
        .handle(&asked_to_let_go_of_the_gone(&board))
        .await
        .expect("letting go should succeed");
    // then: one of them is let go of
    assert_eq!(
        pinned_on(&wired, &board).await.len(),
        2,
        "one idea is let go of in a hearing, as two unpins on one board would collide in one transaction"
    );
    // when: the hearings that follow are done
    wired.settle().await;

    // then: only the idea still there stays on the board
    assert_eq!(
        pinned_on(&wired, &board).await,
        ["idea_3"],
        "the unpin's own event asks the board to let go of the next"
    );
}
