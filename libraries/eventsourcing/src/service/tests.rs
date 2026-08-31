use clock::FixedClock;
use time::OffsetDateTime;

use super::*;
use crate::agent::AgentId;
use crate::memory::InMemoryEventStore;
use crate::patch::Patcher;
use crate::testing::sample::{Sample, SampleCommand, SampleError, SampleEvent};
use crate::testing::sample::{SampleKind, a_sample, recorded, stamped};

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

fn happenings(landed: &Appended<SampleEvent>) -> Vec<SampleEvent> {
    landed
        .events
        .iter()
        .map(|entry| entry.event.clone())
        .collect()
}

fn retitled(to: &str) -> SampleCommand {
    SampleCommand::UpdateTitle(to.to_owned())
}

async fn a_created_sample(service: &EventSourcingService<Sample>) -> AggregateId {
    let id = AggregateId::from("sample_1");
    service
        .begin(&id, a_creation(), &an_author())
        .await
        .expect("creating should succeed");

    id
}

#[tokio::test]
async fn creating_lands_the_first_version() {
    let service = a_workbench();
    let id = AggregateId::from("sample_1");

    let landed = service
        .begin(&id, a_creation(), &an_author())
        .await
        .expect("creating should succeed");

    assert_eq!(landed.version, Version::of(1));
    assert_eq!(
        service
            .latest(&id)
            .await
            .expect("it exists now")
            .state
            .title,
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

    assert_eq!(landed.version, Version::of(2));
    assert_eq!(
        service.latest(&id).await.expect("it exists").state.title,
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
async fn a_command_on_something_that_does_not_exist_is_not_found() {
    let service = a_workbench();
    let missing = AggregateId::from("sample_early");

    let refused = service
        .execute(&missing, retitled("Too soon"), &an_author())
        .await
        .expect_err("nothing exists to update");

    assert_eq!(
        refused,
        ServiceError::NotFound {
            aggregate: missing,
            kind: Sample::KIND,
        },
        "an update to something absent is a missing aggregate, not a domain refusal"
    );
}

#[tokio::test]
async fn beginning_with_a_command_that_does_not_create_is_refused_by_the_aggregate() {
    let service = a_workbench();

    let refused = service
        .begin(
            &AggregateId::from("sample_early"),
            retitled("Too soon"),
            &an_author(),
        )
        .await
        .expect_err("only a creation command can start a stream");

    assert_eq!(refused, ServiceError::Refused(SampleError::NotCreatedYet));
}

#[tokio::test]
async fn beginning_something_that_already_exists_is_a_conflict() {
    let service = a_workbench();
    let id = a_created_sample(&service).await;

    let refused = service
        .begin(&id, a_creation(), &an_author())
        .await
        .expect_err("it is already there");

    assert!(matches!(
        refused,
        ServiceError::Store(StoreError::Outdated { .. })
    ));
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
        service.latest(&id).await.expect("it exists").state.title,
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

    assert_eq!(landed.version, Version::of(2));
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

    assert_eq!(then.state.title, "The Loom");
    assert_eq!(then.version, Version::of(1));
    assert_eq!(now.state.title, "The Silent Loom");
}

#[tokio::test]
async fn many_aggregates_do_not_interfere() {
    let service = a_workbench();
    let one = AggregateId::from("sample_one");
    let other = AggregateId::from("sample_other");

    service
        .begin(&one, a_creation(), &an_author())
        .await
        .expect("creating should succeed");
    service
        .begin(&other, a_creation(), &an_author())
        .await
        .expect("creating should succeed");
    service
        .execute(&other, retitled("Apart"), &an_author())
        .await
        .expect("updating should succeed");

    assert_eq!(
        service.latest(&one).await.expect("exists").state.title,
        "The Loom"
    );
    assert_eq!(
        service.latest(&other).await.expect("exists").state.title,
        "Apart"
    );
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

    assert_eq!(
        landed.version,
        Version::of(3),
        "two events land two versions"
    );
    let state = service.latest(&id).await.expect("exists").state;
    assert_eq!(state.title, "The Silent Loom");
    assert_eq!(state.description, "It remembers.");
}

#[tokio::test]
async fn what_landed_carries_the_events_that_were_appended() {
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

    assert_eq!(
        happenings(&landed),
        vec![
            SampleEvent::TitleUpdated("The Silent Loom".to_owned()),
            SampleEvent::DescriptionUpdated("It remembers.".to_owned()),
        ],
        "a caller that has to publish what happened cannot work it out from a version alone"
    );
    assert_eq!(
        landed
            .events
            .iter()
            .map(|entry| entry.metadata.version)
            .collect::<Vec<_>>(),
        vec![Version::of(2), Version::of(3)],
        "each event has its own version, and the last one alone cannot say so"
    );
    assert!(!landed.changed_nothing());
}

#[tokio::test]
async fn a_snapshot_is_not_something_that_happened() {
    let service = a_workbench();
    let id = a_created_sample(&service).await;

    for counted in 2..100 {
        service
            .execute(&id, retitled(&format!("Title {counted}")), &an_author())
            .await
            .expect("updating should succeed");
    }

    let landed = service
        .execute(&id, retitled("Title 100"), &an_author())
        .await
        .expect("updating should succeed");

    assert_eq!(
        landed.version,
        Version::of(101),
        "the snapshot still moved the version along"
    );
    assert_eq!(
        happenings(&landed),
        vec![SampleEvent::TitleUpdated("Title 100".to_owned())],
        "one command happened once, and collapsing the log is not news"
    );
    assert!(
        landed
            .events
            .iter()
            .all(|entry| !entry.metadata.is_snapshot),
        "and the snapshot is not among what was published"
    );
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
        landed.version,
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
        service.latest(&id).await.expect("exists").state.title,
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
        .compact(&id, &an_author())
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
        service.latest(&id).await.expect("exists").state.title,
        "The Silent Loom",
        "a collapsed stream must still describe the same aggregate"
    );
}

#[tokio::test]
async fn a_collapsed_aggregate_carries_on_from_the_snapshot() {
    let service = a_workbench();
    let id = a_created_sample(&service).await;
    service
        .compact(&id, &an_author())
        .await
        .expect("collapsing should succeed");

    let landed = service
        .execute(&id, retitled("After the compact"), &an_author())
        .await
        .expect("a collapsed aggregate still accepts commands");

    assert_eq!(landed.version, Version::of(3));
    assert_eq!(
        service.latest(&id).await.expect("exists").state.title,
        "After the compact"
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

#[tokio::test]
async fn a_stale_version_below_a_collapsed_stream_is_outdated_not_missing() {
    let service = a_workbench();
    let id = a_created_sample(&service).await;
    service
        .execute(&id, retitled("Second"), &an_author())
        .await
        .expect("updating should succeed");
    service
        .compact(&id, &an_author())
        .await
        .expect("collapsing should succeed");

    let refused = service
        .execute_at(&id, Version::of(1), retitled("Stale"), &an_author())
        .await
        .expect_err("version 1 was pruned away, but the aggregate is very much there");

    assert!(
        matches!(refused, ServiceError::Store(StoreError::Outdated { .. })),
        "a version the snapshot swallowed must not read as a missing aggregate, got {refused:?}"
    );
}

#[tokio::test]
async fn an_event_stored_at_an_older_version_is_upgraded_before_the_aggregate_sees_it() {
    let store = a_store();
    let service = a_service(store.clone());
    let id = AggregateId::from("sample_ancient");

    store
        .append(
            &id,
            Sample::KIND,
            Version::ZERO,
            &recorded(
                &id,
                vec![SampleEvent::CreatedBeforeKinds {
                    title: "The Loom".to_owned(),
                    description: "A silent machine.".to_owned(),
                }],
            ),
        )
        .await
        .expect("an old event should still be storable");

    let standing = service
        .latest(&id)
        .await
        .expect("a stream written before kinds existed must still rebuild");

    assert_eq!(standing.state.title, "The Loom");
    assert_eq!(
        standing.state.kind,
        SampleKind::Ordinary,
        "the patch supplies what the old event never carried"
    );
}

#[tokio::test]
async fn an_unpatched_old_event_would_not_rebuild_at_all() {
    let patcher = Patcher::holding(Vec::new());
    let ancient = SampleEvent::CreatedBeforeKinds {
        title: "The Loom".to_owned(),
        description: "A silent machine.".to_owned(),
    };

    let untouched = patcher.patch(ancient.clone());

    assert_eq!(untouched, ancient);
    assert!(
        Sample::from_first(&untouched, &stamped(&a_sample(), 1)).is_none(),
        "the previous test only means something because the aggregate cannot read the old shape"
    );
}

#[tokio::test]
async fn a_current_event_is_left_alone_by_the_patcher() {
    let patcher = Patcher::holding(Sample::patches());
    let current = SampleEvent::Created {
        title: "The Loom".to_owned(),
        description: "A silent machine.".to_owned(),
        kind: SampleKind::Remarkable,
    };

    assert_eq!(
        patcher.patch(current.clone()),
        current,
        "a patch must not fire on the version it produces, or it would loop"
    );
}

#[tokio::test]
async fn an_old_event_still_reads_back_after_a_snapshot_replaces_it() {
    let store = a_store();
    let service = a_service(store.clone());
    let id = AggregateId::from("sample_ancient_collapsed");

    store
        .append(
            &id,
            Sample::KIND,
            Version::ZERO,
            &recorded(
                &id,
                vec![SampleEvent::CreatedBeforeKinds {
                    title: "The Loom".to_owned(),
                    description: "A silent machine.".to_owned(),
                }],
            ),
        )
        .await
        .expect("an old event should still be storable");

    service
        .compact(&id, &an_author())
        .await
        .expect("collapsing a patched stream should succeed");

    assert_eq!(
        service
            .latest(&id)
            .await
            .expect("it still exists")
            .state
            .kind,
        SampleKind::Ordinary,
        "a snapshot taken of a patched aggregate must carry the patched state"
    );
}

#[tokio::test]
async fn an_event_two_versions_old_climbs_the_whole_chain() {
    let store = a_store();
    let service = a_service(store.clone());
    let id = AggregateId::from("sample_very_ancient");

    store
        .append(
            &id,
            Sample::KIND,
            Version::ZERO,
            &recorded(
                &id,
                vec![SampleEvent::CreatedBeforeDescriptions {
                    title: "The Loom".to_owned(),
                }],
            ),
        )
        .await
        .expect("an event from two versions ago should still be storable");

    let standing = service
        .latest(&id)
        .await
        .expect("two patches in a row must carry it all the way to the current shape");

    assert_eq!(standing.state.title, "The Loom");
    assert_eq!(standing.state.description, "");
    assert_eq!(standing.state.kind, SampleKind::Ordinary);
}

#[tokio::test]
async fn patches_are_applied_in_version_order_however_they_were_registered() {
    let backwards = Patcher::holding(Sample::patches());
    let ancient = SampleEvent::CreatedBeforeDescriptions {
        title: "The Loom".to_owned(),
    };

    let climbed = backwards.patch(ancient);

    assert_eq!(
        climbed,
        SampleEvent::Created {
            title: "The Loom".to_owned(),
            description: String::new(),
            kind: SampleKind::Ordinary,
        },
        "the sample registers the newer patch first on purpose, so only sorting gets this right"
    );
}
