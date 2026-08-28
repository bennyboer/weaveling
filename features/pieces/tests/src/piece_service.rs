use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{
    Agent, AgentId, AggregateId, EventStore, InMemoryEventStore, ServiceError, Version,
};
use pieces_catalog::InMemoryPieceCatalog;
use time::{Duration, OffsetDateTime};

use pieces_core::{
    KIND, PassageLink, PieceError, PieceEvent, PieceId, PieceService, PieceServiceError,
    PieceTitle, ProjectLink,
};

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn a_workbench() -> (PieceService, Arc<FixedClock>) {
    let clock = Arc::new(FixedClock::new(at(1_000)));
    let service = PieceService::new(
        Arc::new(InMemoryEventStore::<PieceEvent>::new()),
        Arc::new(InMemoryPieceCatalog::new()),
        clock.clone(),
    );

    (service, clock)
}

fn an_author() -> Agent {
    Agent::User(AgentId::from("author-7"))
}

async fn a_captured_piece(service: &PieceService) -> PieceId {
    service
        .capture("project_1", "The Loom", &an_author())
        .await
        .expect("capturing should succeed")
}

#[tokio::test]
async fn capturing_yields_an_id_the_piece_can_be_fetched_with() {
    let (service, _) = a_workbench();

    let id = a_captured_piece(&service).await;
    let piece = service
        .get(&id.to_string())
        .await
        .expect("the piece exists");

    assert_eq!(piece.state.title().as_str(), "The Loom");
    assert_eq!(piece.state.project(), &ProjectLink::from("project_1"));
    assert_eq!(piece.state.passage(), None);
    assert_eq!(piece.version, Version::of(1));
}

#[tokio::test]
async fn the_client_never_computes_a_piece_id() {
    let (service, _) = a_workbench();

    let id = a_captured_piece(&service).await;

    assert!(
        id.to_string().starts_with("piece_"),
        "an id should say what it is: {id}"
    );
}

#[tokio::test]
async fn two_pieces_captured_in_the_same_instant_still_differ() {
    let (service, _) = a_workbench();

    let one = a_captured_piece(&service).await;
    let other = a_captured_piece(&service).await;

    assert_ne!(one, other, "a fixed clock must not collapse two ideas");
}

#[tokio::test]
async fn a_piece_may_be_captured_with_no_title() {
    let (service, _) = a_workbench();

    let id = service
        .capture("project_1", "", &an_author())
        .await
        .expect("an idea arrives before its name");

    assert!(
        service
            .get(&id.to_string())
            .await
            .expect("it exists")
            .state
            .title()
            .is_untitled()
    );
}

#[tokio::test]
async fn a_title_the_domain_refuses_never_reaches_the_store() {
    let (service, _) = a_workbench();

    let refused = service
        .capture(
            "project_1",
            &"a".repeat(PieceTitle::MAX_CHARS + 1),
            &an_author(),
        )
        .await
        .expect_err("a sprawling title is not a title");

    assert!(matches!(refused, PieceServiceError::InvalidTitle(_)));
}

#[tokio::test]
async fn retitling_changes_what_the_piece_is_called() {
    let (service, _) = a_workbench();
    let id = a_captured_piece(&service).await;

    let landed = service
        .retitle(&id.to_string(), "The Silent Loom", None, &an_author())
        .await
        .expect("retitling should succeed");

    assert_eq!(landed, Version::of(2));
    assert_eq!(
        service
            .get(&id.to_string())
            .await
            .expect("it exists")
            .state
            .title()
            .as_str(),
        "The Silent Loom"
    );
}

#[tokio::test]
async fn retitling_to_the_same_title_is_accepted_and_records_nothing() {
    let (service, _) = a_workbench();
    let id = a_captured_piece(&service).await;

    let landed = service
        .retitle(&id.to_string(), "The Loom", None, &an_author())
        .await
        .expect("an unchanged title is not an error");

    assert_eq!(
        landed,
        Version::of(1),
        "the version must not move when nothing happened"
    );
}

#[tokio::test]
async fn a_passage_can_be_attached_once() {
    let (service, _) = a_workbench();
    let id = a_captured_piece(&service).await;

    service
        .attach_passage(&id.to_string(), "passage_9", None, &an_author())
        .await
        .expect("attaching should succeed");

    assert_eq!(
        service
            .get(&id.to_string())
            .await
            .expect("it exists")
            .state
            .passage(),
        Some(&PassageLink::from("passage_9"))
    );
}

#[tokio::test]
async fn a_second_passage_is_refused() {
    let (service, _) = a_workbench();
    let id = a_captured_piece(&service).await;
    service
        .attach_passage(&id.to_string(), "passage_9", None, &an_author())
        .await
        .expect("the first attaches");

    let refused = service
        .attach_passage(&id.to_string(), "passage_10", None, &an_author())
        .await
        .expect_err("the second must not");

    assert!(matches!(
        refused,
        PieceServiceError::Events(ServiceError::Refused(PieceError::AlreadyHoldsPassage))
    ));
}

#[tokio::test]
async fn a_discarded_piece_refuses_further_changes() {
    let (service, _) = a_workbench();
    let id = a_captured_piece(&service).await;
    service
        .discard(&id.to_string(), None, &an_author())
        .await
        .expect("discarding should succeed");

    let refused = service
        .retitle(&id.to_string(), "Too late", None, &an_author())
        .await
        .expect_err("a discarded piece accepts nothing");

    assert!(matches!(
        refused,
        PieceServiceError::Events(ServiceError::Refused(PieceError::Discarded))
    ));
}

#[tokio::test]
async fn fetching_a_piece_that_was_never_captured_is_not_found() {
    let (service, _) = a_workbench();
    let never = PieceId::generate(at(2_000));

    let missing = service
        .get(&never.to_string())
        .await
        .expect_err("nothing is there");

    assert!(matches!(
        missing,
        PieceServiceError::Events(ServiceError::NotFound { .. })
    ));
}

#[tokio::test]
async fn an_id_of_another_kind_is_refused_before_the_store_is_touched() {
    let (service, _) = a_workbench();
    let theirs = format!("passage_{}", PieceId::generate(at(1_000)).as_uuid());

    assert!(matches!(
        service.get(&theirs).await.expect_err("wrong kind of id"),
        PieceServiceError::InvalidId(_)
    ));
    assert!(matches!(
        service
            .retitle(&theirs, "Nowhere", None, &an_author())
            .await
            .expect_err("wrong kind of id"),
        PieceServiceError::InvalidId(_)
    ));
}

#[tokio::test]
async fn a_piece_captured_later_sorts_after_one_captured_earlier() {
    let (service, clock) = a_workbench();

    let earliest = a_captured_piece(&service).await;
    clock.set(at(2_000));
    let latest = a_captured_piece(&service).await;

    assert!(
        earliest < latest,
        "ids should sort by when the idea arrived"
    );
}

#[tokio::test]
async fn a_piece_is_snapshotted_once_the_threshold_is_reached() {
    let store = Arc::new(InMemoryEventStore::<PieceEvent>::new());
    let service = PieceService::new(
        store.clone(),
        Arc::new(InMemoryPieceCatalog::new()),
        Arc::new(FixedClock::new(at(1_000))),
    );
    let id = service
        .capture("project_1", "The Loom", &an_author())
        .await
        .expect("capturing should succeed");
    service
        .attach_passage(&id.to_string(), "passage_9", None, &an_author())
        .await
        .expect("attaching should succeed");

    for counted in 3..100 {
        service
            .retitle(
                &id.to_string(),
                &format!("Title {counted}"),
                None,
                &an_author(),
            )
            .await
            .expect("retitling should succeed");
    }

    assert!(
        store
            .latest_snapshot(&AggregateId::from(&id), KIND)
            .await
            .expect("looking should succeed")
            .is_none(),
        "ninety-nine events is not yet a hundred"
    );

    let landed = service
        .retitle(&id.to_string(), "Title 100", None, &an_author())
        .await
        .expect("retitling should succeed");

    assert_eq!(
        landed,
        Version::of(101),
        "a snapshot follows the hundredth event"
    );
}

#[tokio::test]
async fn a_piece_survives_on_its_snapshot_alone() {
    let store = Arc::new(InMemoryEventStore::<PieceEvent>::new());
    let service = PieceService::new(
        store.clone(),
        Arc::new(InMemoryPieceCatalog::new()),
        Arc::new(FixedClock::new(at(1_000))),
    );
    let id = service
        .capture("project_1", "The Loom", &an_author())
        .await
        .expect("capturing should succeed");
    service
        .attach_passage(&id.to_string(), "passage_9", None, &an_author())
        .await
        .expect("attaching should succeed");

    for counted in 3..=100 {
        service
            .retitle(
                &id.to_string(),
                &format!("Title {counted}"),
                None,
                &an_author(),
            )
            .await
            .expect("retitling should succeed");
    }

    let key = AggregateId::from(&id);
    let snapshot = store
        .latest_snapshot(&key, KIND)
        .await
        .expect("looking should succeed")
        .expect("a hundred events should have earned a snapshot");
    store
        .prune_through(
            &key,
            KIND,
            snapshot
                .metadata
                .version
                .previous()
                .expect("a snapshot is never the first event"),
        )
        .await
        .expect("pruning should succeed");

    assert_eq!(
        store
            .read_from(&key, KIND, Version::ZERO)
            .await
            .expect("reading should succeed")
            .len(),
        1,
        "everything the snapshot replaced is gone, so what follows can only come from it"
    );

    let piece = service
        .get(&id.to_string())
        .await
        .expect("the snapshot alone should be enough to rebuild the piece")
        .state;

    assert_eq!(piece.title().as_str(), "Title 100");
    assert_eq!(piece.passage(), Some(&PassageLink::from("passage_9")));
    assert_eq!(piece.project(), &ProjectLink::from("project_1"));
    assert!(!piece.is_discarded());
}
