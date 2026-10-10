use std::sync::Arc;

use crate::wiring::{Wired, wired};
use clock::FixedClock;
use eventsourcing::{Agent, AgentId, AggregateId, EventStore, ServiceError, Version};
use time::{Duration, OffsetDateTime};

use ideas_core::{IdeaError, IdeaId, IdeaService, IdeaServiceError, IdeaTitle, KIND, ProjectLink};

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn a_workbench() -> (IdeaService, Arc<FixedClock>) {
    let clock = Arc::new(FixedClock::new(at(1_000)));

    (wired(clock.clone()).ideas, clock)
}

fn an_author() -> Agent {
    Agent::User(AgentId::from("author-7"))
}

async fn a_captured_idea(service: &IdeaService) -> IdeaId {
    service
        .capture("project_1", "The Loom", &an_author())
        .await
        .expect("capturing should succeed")
}

#[tokio::test]
async fn capturing_yields_an_id_the_idea_can_be_fetched_with() {
    let (service, _) = a_workbench();

    let id = a_captured_idea(&service).await;
    let idea = service.get(&id.to_string()).await.expect("the idea exists");

    assert_eq!(idea.state.title().as_str(), "The Loom");
    assert_eq!(idea.state.project(), &ProjectLink::from("project_1"));
    assert_eq!(idea.version, Version::of(1));
}

#[tokio::test]
async fn the_client_never_computes_an_idea_id() {
    let (service, _) = a_workbench();

    let id = a_captured_idea(&service).await;

    assert!(
        id.to_string().starts_with("idea_"),
        "an id should say what it is: {id}"
    );
}

#[tokio::test]
async fn two_ideas_captured_in_the_same_instant_still_differ() {
    let (service, _) = a_workbench();

    let one = a_captured_idea(&service).await;
    let other = a_captured_idea(&service).await;

    assert_ne!(one, other, "a fixed clock must not compact two ideas");
}

#[tokio::test]
async fn an_idea_may_be_captured_with_no_title() {
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
            &"a".repeat(IdeaTitle::MAX_CHARS + 1),
            &an_author(),
        )
        .await
        .expect_err("a sprawling title is not a title");

    assert!(matches!(refused, IdeaServiceError::InvalidTitle(_)));
}

#[tokio::test]
async fn retitling_changes_what_the_idea_is_called() {
    let (service, _) = a_workbench();
    let id = a_captured_idea(&service).await;

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
    let id = a_captured_idea(&service).await;

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
async fn a_discarded_idea_refuses_further_changes() {
    let (service, _) = a_workbench();
    let id = a_captured_idea(&service).await;
    service
        .discard(&id.to_string(), None, &an_author())
        .await
        .expect("discarding should succeed");

    let refused = service
        .retitle(&id.to_string(), "Too late", None, &an_author())
        .await
        .expect_err("a discarded idea accepts nothing");

    assert!(matches!(
        refused,
        IdeaServiceError::Events(ServiceError::Refused(IdeaError::Discarded))
    ));
}

#[tokio::test]
async fn fetching_an_idea_that_was_never_captured_is_not_found() {
    let (service, _) = a_workbench();
    let never = IdeaId::generate(at(2_000));

    let missing = service
        .get(&never.to_string())
        .await
        .expect_err("nothing is there");

    assert!(matches!(
        missing,
        IdeaServiceError::Events(ServiceError::NotFound { .. })
    ));
}

#[tokio::test]
async fn an_id_of_another_kind_is_refused_before_the_store_is_touched() {
    let (service, _) = a_workbench();
    let theirs = format!("scene_{}", IdeaId::generate(at(1_000)).as_uuid());

    assert!(matches!(
        service.get(&theirs).await.expect_err("wrong kind of id"),
        IdeaServiceError::InvalidId(_)
    ));
    assert!(matches!(
        service
            .retitle(&theirs, "Nowhere", None, &an_author())
            .await
            .expect_err("wrong kind of id"),
        IdeaServiceError::InvalidId(_)
    ));
}

#[tokio::test]
async fn an_idea_captured_later_sorts_after_one_captured_earlier() {
    let (service, clock) = a_workbench();

    let earliest = a_captured_idea(&service).await;
    clock.set(at(2_000));
    let latest = a_captured_idea(&service).await;

    assert!(
        earliest < latest,
        "ids should sort by when the idea arrived"
    );
}

#[tokio::test]
async fn an_idea_is_snapshotted_once_the_threshold_is_reached() {
    let Wired {
        ideas: service,
        store,
        ..
    } = wired(Arc::new(FixedClock::new(at(1_000))));
    let id = service
        .capture("project_1", "The Loom", &an_author())
        .await
        .expect("capturing should succeed");
    service
        .retitle(&id.to_string(), "Title 2", None, &an_author())
        .await
        .expect("retitling should succeed");

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
async fn an_idea_survives_on_its_snapshot_alone() {
    let Wired {
        ideas: service,
        store,
        ..
    } = wired(Arc::new(FixedClock::new(at(1_000))));
    let id = service
        .capture("project_1", "The Loom", &an_author())
        .await
        .expect("capturing should succeed");
    service
        .retitle(&id.to_string(), "Title 2", None, &an_author())
        .await
        .expect("retitling should succeed");

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

    let idea = service
        .get(&id.to_string())
        .await
        .expect("the snapshot alone should be enough to rebuild the idea")
        .state;

    assert_eq!(idea.title().as_str(), "Title 100");
    assert_eq!(idea.project(), &ProjectLink::from("project_1"));
    assert!(!idea.is_discarded());
}
