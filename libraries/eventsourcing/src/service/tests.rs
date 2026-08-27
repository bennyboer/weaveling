use clock::FixedClock;
use time::OffsetDateTime;

use super::*;
use crate::agent::AgentId;
use crate::memory::InMemoryEventStore;
use crate::testing::sample::{Sample, SampleCommand, SampleError, SampleEvent};

fn a_store() -> Arc<InMemoryEventStore<SampleEvent>> {
    Arc::new(InMemoryEventStore::new())
}

fn a_service(store: Arc<InMemoryEventStore<SampleEvent>>) -> EventSourcingService<Sample> {
    EventSourcingService::new(store, Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)))
}

fn a_workbench() -> EventSourcingService<Sample> {
    a_service(a_store())
}

fn a_creation() -> SampleCommand {
    SampleCommand::Create {
        title: "The Loom".to_owned(),
        description: "A silent machine.".to_owned(),
    }
}

fn an_author() -> Agent {
    Agent::User(AgentId::from("author-7"))
}

fn retitled(to: &str) -> SampleCommand {
    SampleCommand::UpdateTitle(to.to_owned())
}

async fn a_created_sample(service: &EventSourcingService<Sample>) -> AggregateId {
    let id = AggregateId::from("sample_1");
    service
        .execute(&id, a_creation(), &an_author())
        .await
        .expect("creating should succeed");

    id
}

#[tokio::test]
async fn creating_lands_the_first_version() {
    let service = a_workbench();
    let id = AggregateId::from("sample_1");

    let landed = service
        .execute(&id, a_creation(), &an_author())
        .await
        .expect("creating should succeed");

    assert_eq!(landed, Version::of(1));
    assert_eq!(
        service.latest(&id).await.expect("it exists now").title,
        "The Loom"
    );
}

#[tokio::test]
async fn a_further_command_lands_the_next_version() {
    let service = a_workbench();
    let id = a_created_sample(&service).await;

    let landed = service
        .execute(&id, retitled("The Silent Loom"), &an_author())
        .await
        .expect("updating should succeed");

    assert_eq!(landed, Version::of(2));
    assert_eq!(
        service.latest(&id).await.expect("it exists").title,
        "The Silent Loom"
    );
}

#[tokio::test]
async fn asking_for_what_was_never_created_is_not_found() {
    let service = a_workbench();
    let missing = AggregateId::from("sample_nobody");

    assert_eq!(
        service
            .latest(&missing)
            .await
            .expect_err("nothing is there"),
        ServiceError::NotFound {
            aggregate: missing,
            kind: Sample::KIND,
        }
    );
}

#[tokio::test]
async fn a_command_before_creation_is_refused_by_the_aggregate() {
    let service = a_workbench();

    let refused = service
        .execute(
            &AggregateId::from("sample_early"),
            retitled("Too soon"),
            &an_author(),
        )
        .await
        .expect_err("nothing exists to update");

    assert_eq!(refused, ServiceError::Refused(SampleError::NotCreatedYet));
}

#[tokio::test]
async fn a_command_at_a_stale_version_is_refused() {
    let service = a_workbench();
    let id = a_created_sample(&service).await;
    service
        .execute(&id, retitled("Second"), &an_author())
        .await
        .expect("updating should succeed");

    let refused = service
        .execute_at(&id, Version::of(1), retitled("Stale"), &an_author())
        .await
        .expect_err("the stream has moved on");

    assert!(matches!(
        refused,
        ServiceError::Store(StoreError::Outdated { .. })
    ));
    assert_eq!(
        service.latest(&id).await.expect("it exists").title,
        "Second",
        "a refused command must not change the aggregate"
    );
}

#[tokio::test]
async fn a_command_at_the_current_version_is_accepted() {
    let service = a_workbench();
    let id = a_created_sample(&service).await;

    let landed = service
        .execute_at(
            &id,
            Version::of(1),
            retitled("The Silent Loom"),
            &an_author(),
        )
        .await
        .expect("the caller was up to date");

    assert_eq!(landed, Version::of(2));
}

#[tokio::test]
async fn commands_after_deletion_are_refused() {
    let service = a_workbench();
    let id = a_created_sample(&service).await;
    service
        .execute(&id, SampleCommand::Delete, &an_author())
        .await
        .expect("deleting should succeed");

    let refused = service
        .execute(&id, retitled("Too late"), &an_author())
        .await
        .expect_err("a deleted sample accepts nothing");

    assert_eq!(refused, ServiceError::Refused(SampleError::Deleted));
}

#[tokio::test]
async fn an_older_version_can_still_be_read() {
    let service = a_workbench();
    let id = a_created_sample(&service).await;
    service
        .execute(&id, retitled("The Silent Loom"), &an_author())
        .await
        .expect("updating should succeed");

    let then = service
        .as_of(&id, Version::of(1))
        .await
        .expect("version 1 existed");
    let now = service.latest(&id).await.expect("it exists");

    assert_eq!(then.title, "The Loom");
    assert_eq!(now.title, "The Silent Loom");
}

#[tokio::test]
async fn many_aggregates_do_not_interfere() {
    let service = a_workbench();
    let one = AggregateId::from("sample_one");
    let other = AggregateId::from("sample_other");

    service
        .execute(&one, a_creation(), &an_author())
        .await
        .expect("creating should succeed");
    service
        .execute(&other, a_creation(), &an_author())
        .await
        .expect("creating should succeed");
    service
        .execute(&other, retitled("Apart"), &an_author())
        .await
        .expect("updating should succeed");

    assert_eq!(
        service.latest(&one).await.expect("exists").title,
        "The Loom"
    );
    assert_eq!(service.latest(&other).await.expect("exists").title, "Apart");
}

#[tokio::test]
async fn one_command_may_land_several_events_at_once() {
    let service = a_workbench();
    let id = a_created_sample(&service).await;

    let landed = service
        .execute(
            &id,
            SampleCommand::Rewrite {
                title: "The Silent Loom".to_owned(),
                description: "It remembers.".to_owned(),
            },
            &an_author(),
        )
        .await
        .expect("rewriting should succeed");

    assert_eq!(landed, Version::of(3), "two events land two versions");
    let state = service.latest(&id).await.expect("exists");
    assert_eq!(state.title, "The Silent Loom");
    assert_eq!(state.description, "It remembers.");
}

#[tokio::test]
async fn a_snapshot_is_taken_once_the_threshold_is_reached() {
    let store = a_store();
    let service = a_service(store.clone());
    let id = a_created_sample(&service).await;

    for counted in 2..100 {
        service
            .execute(&id, retitled(&format!("Title {counted}")), &an_author())
            .await
            .expect("updating should succeed");
    }

    assert!(
        store
            .latest_snapshot(&id, Sample::KIND)
            .await
            .expect("looking should succeed")
            .is_none(),
        "ninety-nine events is not yet a hundred"
    );

    let landed = service
        .execute(&id, retitled("Title 100"), &an_author())
        .await
        .expect("updating should succeed");

    assert_eq!(
        landed,
        Version::of(101),
        "the hundredth event should be followed by a snapshot"
    );
    assert_eq!(
        store
            .latest_snapshot(&id, Sample::KIND)
            .await
            .expect("looking should succeed")
            .expect("a snapshot was due")
            .metadata
            .version,
        Version::of(101)
    );
    assert_eq!(
        service.latest(&id).await.expect("exists").title,
        "Title 100",
        "a snapshot must not change what the aggregate says"
    );
}

#[tokio::test]
async fn collapsing_leaves_only_a_snapshot_behind() {
    let store = a_store();
    let service = a_service(store.clone());
    let id = a_created_sample(&service).await;
    service
        .execute(&id, retitled("The Silent Loom"), &an_author())
        .await
        .expect("updating should succeed");

    let taken = service
        .collapse(&id, &an_author())
        .await
        .expect("collapsing should succeed");

    assert_eq!(taken, Version::of(3));
    let stream = store
        .read_from(&id, Sample::KIND, Version::ZERO)
        .await
        .expect("reading should succeed");
    assert_eq!(stream.len(), 1, "everything the snapshot replaced is gone");
    assert!(stream[0].metadata.is_snapshot);
    assert_eq!(
        service.latest(&id).await.expect("exists").title,
        "The Silent Loom",
        "a collapsed stream must still describe the same aggregate"
    );
}

#[tokio::test]
async fn a_collapsed_aggregate_carries_on_from_the_snapshot() {
    let service = a_workbench();
    let id = a_created_sample(&service).await;
    service
        .collapse(&id, &an_author())
        .await
        .expect("collapsing should succeed");

    let landed = service
        .execute(&id, retitled("After the collapse"), &an_author())
        .await
        .expect("a collapsed aggregate still accepts commands");

    assert_eq!(landed, Version::of(3));
    assert_eq!(
        service.latest(&id).await.expect("exists").title,
        "After the collapse"
    );
}

#[tokio::test]
async fn who_acted_and_when_is_recorded() {
    let store = a_store();
    let service = a_service(store.clone());
    let id = a_created_sample(&service).await;

    let stream = store
        .read_from(&id, Sample::KIND, Version::ZERO)
        .await
        .expect("reading should succeed");

    assert_eq!(stream[0].metadata.agent, an_author());
    assert_eq!(stream[0].metadata.occurred_at, OffsetDateTime::UNIX_EPOCH);
    assert_eq!(stream[0].metadata.kind, Sample::KIND);
    assert_eq!(stream[0].metadata.aggregate, id);
}
