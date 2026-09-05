use std::sync::Arc;

use boards_contract::{PIECE_MOVED, PIECE_PINNED, PIECE_UNPINNED, STARTED};
use boards_core::{
    BoardCatalog, BoardError, BoardEvent, BoardId, BoardServiceError, BoardSummary, KIND,
    PieceLink, ProjectLink, Size, Spot,
};
use clock::FixedClock;
use eventsourcing::{
    Agent, AgentId, AggregateId, EventMetadata, EventStore, Recorded, ServiceError, StoreError,
    Version,
};
use messaging::RoutingKey;
use time::{Duration, OffsetDateTime};

use crate::wiring::{Wired, wired};

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn a_piece() -> PieceLink {
    PieceLink::from("piece_1")
}

fn a_key(of: &str) -> RoutingKey {
    RoutingKey::parse(of).expect("a declared routing key is fine")
}

fn an_author() -> Agent {
    Agent::User(AgentId::from("author-7"))
}

fn a_workbench() -> (Wired, Arc<FixedClock>) {
    let clock = Arc::new(FixedClock::new(at(1_000)));

    (wired(clock.clone()), clock)
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
    assert!(opened.standing.state.pieces().is_empty());
}

#[tokio::test]
async fn opening_a_project_whose_board_is_already_catalogued_finds_it() {
    let (wired, _) = a_workbench();
    let known = BoardId::generate(at(500));
    wired
        .catalog
        .remember(&BoardSummary {
            id: known,
            project: ProjectLink::from("project_1"),
        })
        .await
        .expect("remembering should succeed");
    wired
        .store
        .append(
            &AggregateId::from(&known),
            KIND,
            Version::ZERO,
            &[a_start(&known)],
        )
        .await
        .expect("appending should succeed");

    let opened = wired
        .boards
        .open("project_1", &an_author())
        .await
        .expect("opening should succeed");

    assert_eq!(
        opened.id, known,
        "the find branch must take what the catalog offers rather than starting afresh"
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
        "a second open must not leave the author looking at a different board; this one          leans on delivery being synchronous, and over a broker it is the find-or-start race"
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
async fn a_pinned_piece_is_there_when_the_board_is_read_again() {
    let (wired, _) = a_workbench();
    let id = an_open_board(&wired).await;

    wired
        .boards
        .pin(
            &id.to_string(),
            a_piece(),
            Spot::at(120, -40),
            Size::CARD,
            None,
            &an_author(),
        )
        .await
        .expect("pinning should succeed");

    let standing = wired
        .boards
        .get(&id.to_string())
        .await
        .expect("reading should succeed");

    assert_eq!(
        standing.state.spot_of(&"piece_1".into()),
        Some(Spot::at(120, -40))
    );
}

#[tokio::test]
async fn a_moved_piece_keeps_its_new_spot() {
    let (wired, _) = a_workbench();
    let id = an_open_board(&wired).await;
    wired
        .boards
        .pin(
            &id.to_string(),
            a_piece(),
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
            a_piece(),
            Some(Spot::at(300, 20)),
            None,
            None,
            &an_author(),
        )
        .await
        .expect("moving should succeed");

    let standing = wired
        .boards
        .get(&id.to_string())
        .await
        .expect("reading should succeed");
    assert_eq!(
        standing.state.spot_of(&"piece_1".into()),
        Some(Spot::at(300, 20))
    );
}

#[tokio::test]
async fn an_unpinned_piece_leaves_the_board() {
    let (wired, _) = a_workbench();
    let id = an_open_board(&wired).await;
    wired
        .boards
        .pin(
            &id.to_string(),
            a_piece(),
            Spot::at(10, 10),
            Size::CARD,
            None,
            &an_author(),
        )
        .await
        .expect("pinning should succeed");

    wired
        .boards
        .unpin(&id.to_string(), a_piece(), None, &an_author())
        .await
        .expect("unpinning should succeed");

    let standing = wired
        .boards
        .get(&id.to_string())
        .await
        .expect("reading should succeed");
    assert!(standing.state.pieces().is_empty());
}

#[tokio::test]
async fn pinning_the_same_piece_twice_is_refused() {
    let (wired, _) = a_workbench();
    let id = an_open_board(&wired).await;
    wired
        .boards
        .pin(
            &id.to_string(),
            a_piece(),
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
            a_piece(),
            Spot::at(9, 9),
            Size::CARD,
            None,
            &an_author(),
        )
        .await;

    assert!(matches!(
        refused,
        Err(BoardServiceError::Events(ServiceError::Refused(
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
            a_piece(),
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
            a_piece(),
            Some(Spot::at(20, 20)),
            None,
            Some(Version::of(1)),
            &an_author(),
        )
        .await;

    assert!(
        matches!(
            refused,
            Err(BoardServiceError::Events(ServiceError::Store(
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
    for quiet in [PIECE_PINNED, PIECE_MOVED, PIECE_UNPINNED] {
        assert!(
            !wired.projector.hears(&a_key(quiet)),
            "which board a project has cannot change on a drop, so {quiet} must not cost a projection write"
        );
    }
}

#[tokio::test]
async fn a_board_nobody_started_is_not_found() {
    let (wired, _) = a_workbench();
    let never_started = BoardId::generate(at(1_000));

    let refused = wired.boards.get(&never_started.to_string()).await;

    assert!(matches!(
        refused,
        Err(BoardServiceError::Events(ServiceError::NotFound { .. }))
    ));
}

#[tokio::test]
async fn an_id_that_is_not_a_board_is_refused() {
    let (wired, _) = a_workbench();

    let refused = wired.boards.get("piece_031VkO0hnpeQZUiAB7nDma").await;

    assert!(matches!(refused, Err(BoardServiceError::InvalidId(_))));
}
