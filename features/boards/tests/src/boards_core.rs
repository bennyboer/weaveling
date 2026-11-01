use std::sync::Arc;

use authorizing::AllowingEverything;
use boards_contract::{IDEA_MOVED, IDEA_PINNED, IDEA_UNPINNED, STARTED};
use boards_core::{
    BoardError, BoardEvent, BoardId, BoardsCore, BoardsError, IdeaLink, KIND, ProjectLink, Size,
    Spot,
};
use clock::FixedClock;
use eventsourcing::{
    AccountId, Agent, AggregateContext, AggregateId, EventMetadata, EventStore, FailsAppending,
    Recorded, RefusesAppending, ServiceError, StoreError, Version,
};
use messaging::RoutingKey;
use registry::Registry;
use time::{Duration, OffsetDateTime};
use transacting::{InMemoryTransactions, Transactions};

use crate::wiring;
use crate::wiring::Wired;

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn an_idea() -> IdeaLink {
    IdeaLink::from("idea_1")
}

fn a_key(of: &str) -> RoutingKey {
    RoutingKey::parse(of).expect("a declared routing key is fine")
}

fn an_author() -> Agent {
    Agent::Account(AccountId::from("author-7"))
}

fn a_workbench() -> (Wired, Arc<FixedClock>) {
    let clock = Arc::new(FixedClock::new(at(1_000)));

    (wiring::wired(clock.clone()), clock)
}

async fn an_open_board(wired: &Wired) -> BoardId {
    wired
        .boards
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed")
        .id
}

fn a_start(id: &BoardId) -> Recorded<BoardEvent> {
    Recorded {
        metadata: EventMetadata {
            aggregate: AggregateId::from(id),
            kind: KIND,
            version: Version::of(1),
            agent: an_author(),
            occurred_at: at(500),
            is_snapshot: false,
            context: AggregateContext::default(),
        },
        event: BoardEvent::Started {
            project: ProjectLink::from("project_1"),
        },
    }
}

#[tokio::test]
async fn opening_a_project_that_never_had_a_board_starts_one() {
    let (wired, _) = a_workbench();

    let opened = wired
        .boards
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed");

    assert_eq!(opened.standing.version, Version::of(1));
    assert_eq!(
        opened.standing.state.project(),
        &ProjectLink::from("project_1")
    );
    assert!(opened.standing.state.ideas().is_empty());
}

#[tokio::test]
async fn opening_a_project_whose_board_is_already_claimed_finds_it() {
    let (wired, _) = a_workbench();
    let known = BoardId::generate(at(500));
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    wired
        .registry
        .claim(
            &mut transaction,
            KIND.as_str(),
            "project_1",
            &known.to_string(),
        )
        .await
        .expect("claiming should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    wired
        .store
        .append(
            &mut transaction,
            &AggregateId::from(&known),
            KIND,
            Version::ZERO,
            &[a_start(&known)],
        )
        .await
        .expect("appending should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    let opened = wired
        .boards
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed");

    assert_eq!(
        opened.id, known,
        "the find branch takes whoever holds the claim rather than starting afresh; the claim is \
         authoritative where the catalog is only a projection"
    );
}

#[tokio::test]
async fn opening_the_same_project_again_finds_the_board_it_already_had() {
    let (wired, clock) = a_workbench();
    let first = an_open_board(&wired).await;
    clock.set(at(2_000));

    let second = an_open_board(&wired).await;

    assert_eq!(
        first, second,
        "a second open must not leave the author looking at a different board; find-or-start reads \
         the catalog, so it cannot see a board whose projection has not landed yet"
    );
}

#[tokio::test]
async fn each_project_gets_a_board_of_its_own() {
    let (wired, clock) = a_workbench();
    let mine = an_open_board(&wired).await;
    clock.set(at(2_000));

    let theirs = wired
        .boards
        .open("project_2", &an_author())
        .await
        .expect("opening should succeed");

    assert_ne!(mine, theirs.id);
    assert_eq!(
        theirs.standing.state.project(),
        &ProjectLink::from("project_2")
    );
}

#[tokio::test]
async fn a_pinned_idea_is_there_when_the_board_is_read_again() {
    let (wired, _) = a_workbench();
    let id = an_open_board(&wired).await;

    wired
        .boards
        .pin(
            &id.to_string(),
            an_idea(),
            Spot::at(120, -40),
            Size::CARD,
            None,
            &an_author(),
        )
        .await
        .expect("pinning should succeed");

    let standing = wired
        .boards
        .read(&id.to_string(), &an_author())
        .await
        .expect("reading should succeed");

    assert_eq!(
        standing.state.spot_of(&"idea_1".into()),
        Some(Spot::at(120, -40))
    );
}

#[tokio::test]
async fn a_moved_idea_keeps_its_new_spot() {
    let (wired, _) = a_workbench();
    let id = an_open_board(&wired).await;
    wired
        .boards
        .pin(
            &id.to_string(),
            an_idea(),
            Spot::at(10, 10),
            Size::CARD,
            None,
            &an_author(),
        )
        .await
        .expect("pinning should succeed");

    wired
        .boards
        .reshape(
            &id.to_string(),
            an_idea(),
            Some(Spot::at(300, 20)),
            None,
            None,
            &an_author(),
        )
        .await
        .expect("moving should succeed");

    let standing = wired
        .boards
        .read(&id.to_string(), &an_author())
        .await
        .expect("reading should succeed");
    assert_eq!(
        standing.state.spot_of(&"idea_1".into()),
        Some(Spot::at(300, 20))
    );
}

#[tokio::test]
async fn an_unpinned_idea_leaves_the_board() {
    let (wired, _) = a_workbench();
    let id = an_open_board(&wired).await;
    wired
        .boards
        .pin(
            &id.to_string(),
            an_idea(),
            Spot::at(10, 10),
            Size::CARD,
            None,
            &an_author(),
        )
        .await
        .expect("pinning should succeed");

    wired
        .boards
        .unpin(&id.to_string(), an_idea(), None, &an_author())
        .await
        .expect("unpinning should succeed");

    let standing = wired
        .boards
        .read(&id.to_string(), &an_author())
        .await
        .expect("reading should succeed");
    assert!(standing.state.ideas().is_empty());
}

#[tokio::test]
async fn pinning_the_same_idea_twice_is_refused() {
    let (wired, _) = a_workbench();
    let id = an_open_board(&wired).await;
    wired
        .boards
        .pin(
            &id.to_string(),
            an_idea(),
            Spot::ORIGIN,
            Size::CARD,
            None,
            &an_author(),
        )
        .await
        .expect("pinning should succeed");

    let refused = wired
        .boards
        .pin(
            &id.to_string(),
            an_idea(),
            Spot::at(9, 9),
            Size::CARD,
            None,
            &an_author(),
        )
        .await;

    assert!(matches!(
        refused,
        Err(BoardsError::Events(ServiceError::Refused(
            BoardError::AlreadyPinned
        )))
    ));
}

#[tokio::test]
async fn a_move_against_a_stale_version_is_refused() {
    let (wired, _) = a_workbench();
    let id = an_open_board(&wired).await;
    wired
        .boards
        .pin(
            &id.to_string(),
            an_idea(),
            Spot::at(10, 10),
            Size::CARD,
            None,
            &an_author(),
        )
        .await
        .expect("pinning should succeed");

    let refused = wired
        .boards
        .reshape(
            &id.to_string(),
            an_idea(),
            Some(Spot::at(20, 20)),
            None,
            Some(Version::of(1)),
            &an_author(),
        )
        .await;

    assert!(
        matches!(
            refused,
            Err(BoardsError::Events(ServiceError::Store(
                StoreError::Outdated { .. }
            )))
        ),
        "two authors dropping the same card must not silently overwrite one another"
    );
}

#[tokio::test]
async fn the_catalog_projector_is_woken_only_by_a_board_being_started() {
    let (wired, _) = a_workbench();

    assert!(
        wired.projector.hears(&a_key(STARTED)),
        "it has to hear a start"
    );
    for quiet in [IDEA_PINNED, IDEA_MOVED, IDEA_UNPINNED] {
        assert!(
            !wired.projector.hears(&a_key(quiet)),
            "which board a project has cannot change on a drop, so {quiet} must not cost a \
             projection write"
        );
    }
}

#[tokio::test]
async fn a_board_nobody_started_is_not_found() {
    let (wired, _) = a_workbench();
    let never_started = BoardId::generate(at(1_000));

    let refused = wired
        .boards
        .read(&never_started.to_string(), &an_author())
        .await;

    assert!(matches!(
        refused,
        Err(BoardsError::Events(ServiceError::NotFound { .. }))
    ));
}

#[tokio::test]
async fn an_id_that_is_not_a_board_is_refused() {
    let (wired, _) = a_workbench();

    let refused = wired
        .boards
        .read("idea_031VkO0hnpeQZUiAB7nDma", &an_author())
        .await;

    assert!(matches!(refused, Err(BoardsError::InvalidId(_))));
}

#[tokio::test]
async fn a_board_whose_start_cannot_be_kept_leaves_no_claim_behind_and_is_started_next_time() {
    let (wired, clock) = a_workbench();
    let refusing = BoardsCore::new(
        Arc::new(RefusesAppending(wired.store.clone())),
        Arc::new(InMemoryTransactions::new()),
        wired.catalog.clone(),
        wired.registry.clone(),
        Arc::new(AllowingEverything),
        clock,
    );

    let refused = refusing.open("project_1", &an_author()).await;
    let held_after_refusal = wired
        .registry
        .holder(KIND.as_str(), "project_1")
        .await
        .expect("asking should succeed");
    let opened = an_open_board(&wired).await;

    assert!(refused.is_err());
    assert_eq!(
        held_after_refusal, None,
        "a claim kept for a board that never started would point every later open at nothing"
    );
    assert_eq!(
        wired
            .registry
            .holder(KIND.as_str(), "project_1")
            .await
            .expect("asking should succeed"),
        Some(opened.to_string())
    );
}

async fn an_idea_pinned_while_losing_races(
    losses: u8,
    expected: Option<Version>,
) -> (Result<Version, BoardsError>, bool) {
    let (wired, clock) = a_workbench();
    let board = an_open_board(&wired).await;
    let losing = BoardsCore::new(
        Arc::new(FailsAppending::losing_races(wired.store.clone(), losses)),
        Arc::new(InMemoryTransactions::new()),
        wired.catalog.clone(),
        wired.registry.clone(),
        Arc::new(AllowingEverything),
        clock,
    );

    let pinned = losing
        .pin(
            &board.to_string(),
            an_idea(),
            Spot::ORIGIN,
            Size::CARD,
            expected,
            &an_author(),
        )
        .await
        .map(|standing| standing.version);
    let on_board = wired
        .boards
        .read(&board.to_string(), &an_author())
        .await
        .expect("the board is there")
        .state
        .ideas()
        .iter()
        .any(|positioned| positioned.idea == an_idea());

    (pinned, on_board)
}

#[tokio::test]
async fn a_pin_that_lost_a_race_is_tried_again_and_lands() {
    let (pinned, on_board) = an_idea_pinned_while_losing_races(4, None).await;

    assert_eq!(pinned.expect("the fifth try lands"), Version::of(2));
    assert!(on_board);
}

#[tokio::test]
async fn a_pin_that_lost_every_race_it_may_try_is_a_conflict() {
    let (pinned, on_board) = an_idea_pinned_while_losing_races(5, None).await;

    assert!(matches!(
        pinned,
        Err(BoardsError::Events(ServiceError::Store(
            StoreError::Outdated { .. }
        )))
    ));
    assert!(!on_board);
}

#[tokio::test]
async fn a_pin_aimed_at_a_version_that_lost_a_race_is_not_tried_again() {
    let (pinned, on_board) = an_idea_pinned_while_losing_races(1, Some(Version::of(1))).await;

    assert!(
        matches!(
            pinned,
            Err(BoardsError::Events(ServiceError::Store(
                StoreError::Outdated { .. }
            )))
        ) && !on_board,
        "whoever named a version hears that it moved on, a 412, rather than having it papered over"
    );
}
