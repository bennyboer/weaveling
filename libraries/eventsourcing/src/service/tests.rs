use std::sync::Mutex;

use clock::FixedClock;
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, Transaction, Transactions};

use super::*;
use crate::memory::InMemoryEventStore;
use crate::patch::Patcher;
use crate::testing::sample;
use crate::testing::sample::SampleKind;
use crate::testing::sample::{Sample, SampleCommand, SampleError, SampleEvent};

#[async_trait::async_trait]
trait Committed {
    async fn begin_committed(
        &self,
        aggregate: &AggregateId,
        command: SampleCommand,
        agent: &Agent,
    ) -> Outcome<Sample>;

    async fn execute_committed(
        &self,
        aggregate: &AggregateId,
        command: SampleCommand,
        agent: &Agent,
    ) -> Outcome<Sample>;

    async fn execute_at_committed(
        &self,
        aggregate: &AggregateId,
        expected: Version,
        command: SampleCommand,
        agent: &Agent,
    ) -> Outcome<Sample>;

    async fn compact_committed(&self, aggregate: &AggregateId, agent: &Agent) -> Reached<Sample>;
}

#[async_trait::async_trait]
impl Committed for EventSourcingService<Sample> {
    async fn begin_committed(
        &self,
        aggregate: &AggregateId,
        command: SampleCommand,
        agent: &Agent,
    ) -> Outcome<Sample> {
        let mut transaction = InMemoryTransactions::new().begin().await?;
        let appended = self
            .begin(&mut transaction, aggregate, command, agent)
            .await?;
        transaction.finish().await?;

        Ok(appended)
    }

    async fn execute_committed(
        &self,
        aggregate: &AggregateId,
        command: SampleCommand,
        agent: &Agent,
    ) -> Outcome<Sample> {
        transacting::retrying!(&InMemoryTransactions::new(), |transaction| {
            self.execute(transaction, aggregate, command.clone(), agent)
                .await
        })
    }

    async fn execute_at_committed(
        &self,
        aggregate: &AggregateId,
        expected: Version,
        command: SampleCommand,
        agent: &Agent,
    ) -> Outcome<Sample> {
        let mut transaction = InMemoryTransactions::new().begin().await?;
        let appended = self
            .execute_at(&mut transaction, aggregate, Some(expected), command, agent)
            .await?;
        transaction.finish().await?;

        Ok(appended)
    }

    async fn compact_committed(&self, aggregate: &AggregateId, agent: &Agent) -> Reached<Sample> {
        let mut transaction = InMemoryTransactions::new().begin().await?;
        let taken = self.compact(&mut transaction, aggregate, agent).await?;
        transaction.finish().await?;

        Ok(taken)
    }
}

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

fn events_of(landed: &Appended<SampleEvent>) -> Vec<SampleEvent> {
    landed
        .events
        .iter()
        .map(|entry| entry.event.clone())
        .collect()
}

fn a_retitling(to: &str) -> SampleCommand {
    SampleCommand::UpdateTitle(to.to_owned())
}

async fn a_created_sample(service: &EventSourcingService<Sample>) -> AggregateId {
    let id = AggregateId::from("sample_1");
    service
        .begin_committed(&id, a_creation(), &crate::an_author())
        .await
        .expect("creating should succeed");

    id
}

#[tokio::test]
async fn creating_lands_the_first_version() {
    // given: an aggregate never created
    let service = a_workbench();
    let id = AggregateId::from("sample_1");

    // when: it is created
    let landed = service
        .begin_committed(&id, a_creation(), &crate::an_author())
        .await
        .expect("creating should succeed");

    // then: it lands at version one
    assert_eq!(landed.version, Version::of(1));
    // and: reads as created
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
    // given: a created sample
    let service = a_workbench();
    let id = a_created_sample(&service).await;

    // when: it is retitled
    let landed = service
        .execute_committed(&id, a_retitling("The Silent Loom"), &crate::an_author())
        .await
        .expect("updating should succeed");

    // then: that lands the next version
    assert_eq!(landed.version, Version::of(2));
    // and: it reads retitled
    assert_eq!(
        service.latest(&id).await.expect("it exists").state.title,
        "The Silent Loom"
    );
}

#[tokio::test]
async fn asking_for_what_was_never_created_is_not_found() {
    // given: an aggregate never created
    let service = a_workbench();
    let missing = AggregateId::from("sample_nobody");

    // when: it is asked for
    let absent = service
        .latest(&missing)
        .await
        .expect_err("nothing is there");

    // then: it is not found
    assert!(matches!(
        absent,
        ServiceError::NotFound { aggregate, kind } if aggregate == missing && kind == Sample::KIND
    ));
}

#[tokio::test]
async fn a_command_on_something_that_does_not_exist_is_not_found() {
    // given: an aggregate never created
    let service = a_workbench();
    let missing = AggregateId::from("sample_early");

    // when: it is retitled
    let refused = service
        .execute_committed(&missing, a_retitling("Too soon"), &crate::an_author())
        .await
        .expect_err("nothing exists to update");

    // then: it is not found
    assert!(
        matches!(
            refused,
            ServiceError::NotFound { aggregate, kind } if aggregate == missing && kind == Sample::KIND
        ),
        "an update to something absent is a missing aggregate, not a domain refusal"
    );
}

#[tokio::test]
async fn beginning_with_a_command_that_does_not_create_is_refused_by_the_aggregate() {
    // given: nothing created
    let service = a_workbench();

    // when: a sample is begun with a retitling
    let refused = service
        .begin_committed(
            &AggregateId::from("sample_early"),
            a_retitling("Too soon"),
            &crate::an_author(),
        )
        .await
        .expect_err("only a creation command can start a stream");

    // then: the aggregate refuses it as not created yet
    assert!(matches!(
        refused,
        ServiceError::Refused(SampleError::NotCreatedYet)
    ));
}

#[tokio::test]
async fn beginning_something_that_already_exists_is_a_conflict() {
    // given: a created sample
    let service = a_workbench();
    let id = a_created_sample(&service).await;

    // when: it is begun again
    let refused = service
        .begin_committed(&id, a_creation(), &crate::an_author())
        .await
        .expect_err("it is already there");

    // then: that is refused as outdated
    assert!(matches!(
        refused,
        ServiceError::Store(StoreError::Outdated { .. })
    ));
}

#[tokio::test]
async fn a_command_at_a_stale_version_is_refused() {
    // given: a sample retitled once
    let service = a_workbench();
    let id = a_created_sample(&service).await;
    service
        .execute_committed(&id, a_retitling("Second"), &crate::an_author())
        .await
        .expect("updating should succeed");

    // when: a command expecting its first version is executed
    let refused = service
        .execute_at_committed(
            &id,
            Version::of(1),
            a_retitling("Stale"),
            &crate::an_author(),
        )
        .await
        .expect_err("the stream has moved on");

    // then: it is refused as outdated
    assert!(matches!(
        refused,
        ServiceError::Store(StoreError::Outdated { .. })
    ));
    // and: the sample is unchanged
    assert_eq!(
        service.latest(&id).await.expect("it exists").state.title,
        "Second",
        "a refused command must not change the aggregate"
    );
}

#[tokio::test]
async fn a_command_at_the_current_version_is_accepted() {
    // given: a created sample
    let service = a_workbench();
    let id = a_created_sample(&service).await;

    // when: a command expecting its version is executed
    let landed = service
        .execute_at_committed(
            &id,
            Version::of(1),
            a_retitling("The Silent Loom"),
            &crate::an_author(),
        )
        .await
        .expect("the caller was up to date");

    // then: it lands the next version
    assert_eq!(landed.version, Version::of(2));
}

async fn beginning_within(
    service: &EventSourcingService<Sample>,
    command: SampleCommand,
) -> (Transaction, Outcome<Sample>) {
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    let begun = service
        .begin(
            &mut transaction,
            &sample::a_sample(),
            command,
            &crate::an_author(),
        )
        .await;

    (transaction, begun)
}

#[tokio::test]
async fn something_begun_within_a_transaction_lands_when_its_owner_finishes_it() {
    // given: nothing created
    let service = a_workbench();

    // when: a sample is begun within a transaction its owner finishes
    let (transaction, begun) = beginning_within(&service, a_creation()).await;
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    // then: it landed at version one
    assert_eq!(
        begun.expect("beginning should succeed").version,
        Version::of(1)
    );
    // and: it reads as created
    assert_eq!(
        service
            .latest(&sample::a_sample())
            .await
            .expect("it was begun")
            .state
            .title,
        "The Loom"
    );
}

#[tokio::test]
async fn something_begun_within_a_transaction_that_is_dropped_was_never_begun() {
    // given: nothing created
    let service = a_workbench();

    // when: a sample is begun within a transaction that is dropped
    let (transaction, begun) = beginning_within(&service, a_creation()).await;
    begun.expect("beginning should succeed");
    drop(transaction);

    // then: it is not found
    assert!(matches!(
        service.latest(&sample::a_sample()).await,
        Err(ServiceError::NotFound { .. })
    ));
    // and: it can be begun again
    service
        .begin_committed(&sample::a_sample(), a_creation(), &crate::an_author())
        .await
        .expect("it can be begun again");
}

#[tokio::test]
async fn beginning_within_a_transaction_what_already_exists_is_a_conflict() {
    // given: a created sample
    let service = a_workbench();
    service
        .begin_committed(&sample::a_sample(), a_creation(), &crate::an_author())
        .await
        .expect("the sample is created");

    // when: it is begun again within a transaction
    let (_, begun) = beginning_within(&service, a_creation()).await;

    // then: that is refused as outdated
    assert!(matches!(
        begun,
        Err(ServiceError::Store(StoreError::Outdated { .. }))
    ));
}

#[tokio::test]
async fn beginning_within_a_transaction_with_what_does_not_begin_is_refused() {
    // given: nothing created
    let service = a_workbench();

    // when: a sample is begun within a transaction with a retitling
    let (_, begun) = beginning_within(&service, a_retitling("Too soon")).await;

    // then: the aggregate refuses it as not created yet
    assert!(matches!(
        begun,
        Err(ServiceError::Refused(SampleError::NotCreatedYet))
    ));
}

#[tokio::test]
async fn commands_after_deletion_are_refused() {
    // given: a deleted sample
    let service = a_workbench();
    let id = a_created_sample(&service).await;
    service
        .execute_committed(&id, SampleCommand::Delete, &crate::an_author())
        .await
        .expect("deleting should succeed");

    // when: it is retitled
    let refused = service
        .execute_committed(&id, a_retitling("Too late"), &crate::an_author())
        .await
        .expect_err("a deleted sample accepts nothing");

    // then: it is refused as deleted
    assert!(matches!(
        refused,
        ServiceError::Refused(SampleError::Deleted)
    ));
}

#[tokio::test]
async fn an_older_version_can_still_be_read() {
    // given: a retitled sample
    let service = a_workbench();
    let id = a_created_sample(&service).await;
    service
        .execute_committed(&id, a_retitling("The Silent Loom"), &crate::an_author())
        .await
        .expect("updating should succeed");

    // when: its first version and its latest are read
    let then = service
        .as_of(&id, Version::of(1))
        .await
        .expect("version 1 existed");
    let now = service.latest(&id).await.expect("it exists");

    // then: the first reads as it was
    assert_eq!(then.state.title, "The Loom");
    assert_eq!(then.version, Version::of(1));
    // and: the latest as it is
    assert_eq!(now.state.title, "The Silent Loom");
}

#[tokio::test]
async fn many_aggregates_do_not_interfere() {
    // given: nothing created
    let service = a_workbench();
    let one = AggregateId::from("sample_one");
    let other = AggregateId::from("sample_other");

    // when: two samples are created and one of them retitled
    service
        .begin_committed(&one, a_creation(), &crate::an_author())
        .await
        .expect("creating should succeed");
    service
        .begin_committed(&other, a_creation(), &crate::an_author())
        .await
        .expect("creating should succeed");
    service
        .execute_committed(&other, a_retitling("Apart"), &crate::an_author())
        .await
        .expect("updating should succeed");

    // then: the other keeps its title
    assert_eq!(
        service.latest(&one).await.expect("exists").state.title,
        "The Loom"
    );
    // and: the retitled one has its new title
    assert_eq!(
        service.latest(&other).await.expect("exists").state.title,
        "Apart"
    );
}

#[tokio::test]
async fn one_command_may_land_several_events_at_once() {
    // given: a created sample
    let service = a_workbench();
    let id = a_created_sample(&service).await;

    // when: it is rewritten, title and description at once
    let landed = service
        .execute_committed(
            &id,
            SampleCommand::Rewrite {
                title: "The Silent Loom".to_owned(),
                description: "It remembers.".to_owned(),
            },
            &crate::an_author(),
        )
        .await
        .expect("rewriting should succeed");

    // then: two versions land
    assert_eq!(
        landed.version,
        Version::of(3),
        "two events land two versions"
    );
    // and: it reads rewritten
    let state = service.latest(&id).await.expect("exists").state;
    assert_eq!(state.title, "The Silent Loom");
    assert_eq!(state.description, "It remembers.");
}

#[tokio::test]
async fn what_landed_carries_the_events_that_were_appended() {
    // given: a created sample
    let service = a_workbench();
    let id = a_created_sample(&service).await;

    // when: it is rewritten
    let landed = service
        .execute_committed(
            &id,
            SampleCommand::Rewrite {
                title: "The Silent Loom".to_owned(),
                description: "It remembers.".to_owned(),
            },
            &crate::an_author(),
        )
        .await
        .expect("rewriting should succeed");

    // then: what landed carries both events
    assert_eq!(
        events_of(&landed),
        vec![
            SampleEvent::TitleUpdated("The Silent Loom".to_owned()),
            SampleEvent::DescriptionUpdated("It remembers.".to_owned()),
        ],
        "a caller that has to publish what happened cannot work it out from a version alone"
    );
    // and: each with its own version
    assert_eq!(
        landed
            .events
            .iter()
            .map(|entry| entry.metadata.version)
            .collect::<Vec<_>>(),
        vec![Version::of(2), Version::of(3)],
        "each event has its own version, and the last one alone cannot say so"
    );
    // and: says something changed
    assert!(!landed.changed_nothing());
}

#[tokio::test]
async fn a_snapshot_is_not_something_that_happened() {
    // given: a sample one event short of a snapshot
    let service = a_workbench();
    let id = a_created_sample(&service).await;

    for counted in 2..100 {
        service
            .execute_committed(
                &id,
                a_retitling(&format!("Title {counted}")),
                &crate::an_author(),
            )
            .await
            .expect("updating should succeed");
    }

    // when: the event that makes it due lands
    let landed = service
        .execute_committed(&id, a_retitling("Title 100"), &crate::an_author())
        .await
        .expect("updating should succeed");

    // then: the version moved along for the snapshot
    assert_eq!(
        landed.version,
        Version::of(101),
        "the snapshot still moved the version along"
    );
    // and: only the command's event is among what landed
    assert_eq!(
        events_of(&landed),
        vec![SampleEvent::TitleUpdated("Title 100".to_owned())],
        "one command happened once, and collapsing the log is not news"
    );
    // and: no snapshot is
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
    // given: a sample one event short of a snapshot
    let store = a_store();
    let service = a_service(store.clone());
    let id = a_created_sample(&service).await;

    for counted in 2..100 {
        service
            .execute_committed(
                &id,
                a_retitling(&format!("Title {counted}")),
                &crate::an_author(),
            )
            .await
            .expect("updating should succeed");
    }

    // then: it has no snapshot yet
    assert!(
        store
            .latest_snapshot(&id, Sample::KIND)
            .await
            .expect("looking should succeed")
            .is_none(),
        "ninety-nine events is not yet a hundred"
    );

    // when: the event that makes it due lands
    let landed = service
        .execute_committed(&id, a_retitling("Title 100"), &crate::an_author())
        .await
        .expect("updating should succeed");

    // then: a snapshot follows it
    assert_eq!(
        landed.version,
        Version::of(101),
        "the hundredth event should be followed by a snapshot"
    );
    // and: is stored
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
    // and: the sample reads the same
    assert_eq!(
        service.latest(&id).await.expect("exists").state.title,
        "Title 100",
        "a snapshot must not change what the aggregate says"
    );
}

#[tokio::test]
async fn collapsing_leaves_only_a_snapshot_behind() {
    // given: a retitled sample
    let store = a_store();
    let service = a_service(store.clone());
    let id = a_created_sample(&service).await;
    service
        .execute_committed(&id, a_retitling("The Silent Loom"), &crate::an_author())
        .await
        .expect("updating should succeed");

    // when: it is collapsed
    let taken = service
        .compact_committed(&id, &crate::an_author())
        .await
        .expect("collapsing should succeed");

    // then: the snapshot is taken at the next version
    assert_eq!(taken, Version::of(3));
    // and: it alone is left of the stream
    let stream = store
        .read_from(&id, Sample::KIND, Version::ZERO)
        .await
        .expect("reading should succeed");
    assert_eq!(stream.len(), 1, "everything the snapshot replaced is gone");
    assert!(stream[0].metadata.is_snapshot);
    // and: the sample reads the same
    assert_eq!(
        service.latest(&id).await.expect("exists").state.title,
        "The Silent Loom",
        "a collapsed stream must still describe the same aggregate"
    );
}

#[tokio::test]
async fn a_collapsed_aggregate_carries_on_from_the_snapshot() {
    // given: a collapsed sample
    let service = a_workbench();
    let id = a_created_sample(&service).await;
    service
        .compact_committed(&id, &crate::an_author())
        .await
        .expect("collapsing should succeed");

    // when: it is retitled
    let landed = service
        .execute_committed(&id, a_retitling("After the compact"), &crate::an_author())
        .await
        .expect("a collapsed aggregate still accepts commands");

    // then: that lands after the snapshot
    assert_eq!(landed.version, Version::of(3));
    // and: it reads retitled
    assert_eq!(
        service.latest(&id).await.expect("exists").state.title,
        "After the compact"
    );
}

#[tokio::test]
async fn who_acted_and_when_is_recorded() {
    // when: an author creates a sample
    let store = a_store();
    let service = a_service(store.clone());
    let id = a_created_sample(&service).await;

    // then: its event records who acted, when, on what kind and which aggregate
    let stream = store
        .read_from(&id, Sample::KIND, Version::ZERO)
        .await
        .expect("reading should succeed");

    assert_eq!(stream[0].metadata.agent, crate::an_author());
    assert_eq!(stream[0].metadata.occurred_at, OffsetDateTime::UNIX_EPOCH);
    assert_eq!(stream[0].metadata.kind, Sample::KIND);
    assert_eq!(stream[0].metadata.aggregate, id);
}

#[tokio::test]
async fn a_stale_version_below_a_collapsed_stream_is_outdated_not_missing() {
    // given: a retitled sample, collapsed
    let service = a_workbench();
    let id = a_created_sample(&service).await;
    service
        .execute_committed(&id, a_retitling("Second"), &crate::an_author())
        .await
        .expect("updating should succeed");
    service
        .compact_committed(&id, &crate::an_author())
        .await
        .expect("collapsing should succeed");

    // when: a command expecting its first version is executed
    let refused = service
        .execute_at_committed(
            &id,
            Version::of(1),
            a_retitling("Stale"),
            &crate::an_author(),
        )
        .await
        .expect_err("version 1 was pruned away, but the aggregate is very much there");

    // then: it is refused as outdated, not as missing
    assert!(
        matches!(refused, ServiceError::Store(StoreError::Outdated { .. })),
        "a version the snapshot swallowed must not read as a missing aggregate, got {refused:?}"
    );
}

#[tokio::test]
async fn an_event_stored_at_an_older_version_is_upgraded_before_the_aggregate_sees_it() {
    // given: an event stored in its shape from before kinds
    let store = a_store();
    let service = a_service(store.clone());
    let id = AggregateId::from("sample_ancient");

    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    store
        .append(
            &mut transaction,
            &id,
            Sample::KIND,
            Version::ZERO,
            &sample::recorded(
                &id,
                vec![SampleEvent::CreatedBeforeKinds {
                    title: "The Loom".to_owned(),
                    description: "A silent machine.".to_owned(),
                }],
            ),
        )
        .await
        .expect("an old event should still be storable");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    // when: its aggregate is rebuilt
    let standing = service
        .latest(&id)
        .await
        .expect("a stream written before kinds existed must still rebuild");

    // then: it reads as stored
    assert_eq!(standing.state.title, "The Loom");
    // and: with the kind the patch supplies
    assert_eq!(
        standing.state.kind,
        SampleKind::Ordinary,
        "the patch supplies what the old event never carried"
    );
}

#[tokio::test]
async fn an_unpatched_old_event_would_not_rebuild_at_all() {
    // given: a patcher holding no patches
    let patcher = Patcher::holding(Vec::new());
    // and: an event from before kinds
    let ancient = SampleEvent::CreatedBeforeKinds {
        title: "The Loom".to_owned(),
        description: "A silent machine.".to_owned(),
    };

    // when: the event is patched
    let untouched = patcher.patch(ancient.clone());

    // then: it is untouched
    assert_eq!(untouched, ancient);
    // and: gives birth to nothing
    assert!(
        Sample::from_first(&untouched, &sample::stamped(&sample::a_sample(), 1)).is_none(),
        "the previous test only means something because the aggregate cannot read the old shape"
    );
}

#[tokio::test]
async fn a_current_event_is_left_alone_by_the_patcher() {
    // given: the sample's patcher
    let patcher = Patcher::holding(Sample::patches());
    // and: an event of the current shape
    let current = SampleEvent::Created {
        title: "The Loom".to_owned(),
        description: "A silent machine.".to_owned(),
        kind: SampleKind::Remarkable,
    };

    // then: patching leaves it alone
    assert_eq!(
        patcher.patch(current.clone()),
        current,
        "a patch must not fire on the version it produces, or it would loop"
    );
}

#[tokio::test]
async fn an_old_event_still_reads_back_after_a_snapshot_replaces_it() {
    // given: an event stored in its shape from before kinds
    let store = a_store();
    let service = a_service(store.clone());
    let id = AggregateId::from("sample_ancient_collapsed");

    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    store
        .append(
            &mut transaction,
            &id,
            Sample::KIND,
            Version::ZERO,
            &sample::recorded(
                &id,
                vec![SampleEvent::CreatedBeforeKinds {
                    title: "The Loom".to_owned(),
                    description: "A silent machine.".to_owned(),
                }],
            ),
        )
        .await
        .expect("an old event should still be storable");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    // when: its aggregate is collapsed
    service
        .compact_committed(&id, &crate::an_author())
        .await
        .expect("collapsing a patched stream should succeed");

    // then: it reads with the kind the patch supplied
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
    // given: an event stored in its shape from before descriptions
    let store = a_store();
    let service = a_service(store.clone());
    let id = AggregateId::from("sample_very_ancient");

    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    store
        .append(
            &mut transaction,
            &id,
            Sample::KIND,
            Version::ZERO,
            &sample::recorded(
                &id,
                vec![SampleEvent::CreatedBeforeDescriptions {
                    title: "The Loom".to_owned(),
                }],
            ),
        )
        .await
        .expect("an event from two versions ago should still be storable");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    // when: its aggregate is rebuilt
    let standing = service
        .latest(&id)
        .await
        .expect("two patches in a row must carry it all the way to the current shape");

    // then: it reads as stored
    assert_eq!(standing.state.title, "The Loom");
    // and: with what both patches supply
    assert_eq!(standing.state.description, "");
    assert_eq!(standing.state.kind, SampleKind::Ordinary);
}

#[tokio::test]
async fn patches_are_applied_in_version_order_however_they_were_registered() {
    // given: the sample's patcher, its newer patch registered first
    let backwards = Patcher::holding(Sample::patches());
    // and: an event from before descriptions
    let ancient = SampleEvent::CreatedBeforeDescriptions {
        title: "The Loom".to_owned(),
    };

    // when: the event is patched
    let climbed = backwards.patch(ancient);

    // then: it reaches the current shape
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

struct BalksOnce {
    inner: Arc<InMemoryEventStore<SampleEvent>>,
    balks: Mutex<u8>,
}

impl BalksOnce {
    fn wrapping(inner: Arc<InMemoryEventStore<SampleEvent>>, times: u8) -> Arc<Self> {
        Arc::new(Self {
            inner,
            balks: Mutex::new(times),
        })
    }

    fn balked(&self) -> bool {
        let mut left = self.balks.lock().expect("nothing panics holding this");

        if *left == 0 {
            return false;
        }
        *left -= 1;

        true
    }
}

#[async_trait::async_trait]
impl EventStore<SampleEvent> for BalksOnce {
    async fn append(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        expected: Version,
        stream: &[Recorded<SampleEvent>],
    ) -> Result<(), StoreError> {
        if self.balked() {
            return Err(StoreError::outdated(aggregate, kind, expected));
        }

        self.inner
            .append(transaction, aggregate, kind, expected, stream)
            .await
    }

    async fn read_from(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
    ) -> Result<Vec<Recorded<SampleEvent>>, StoreError> {
        self.inner.read_from(aggregate, kind, from).await
    }

    async fn read_from_within(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
    ) -> Result<Vec<Recorded<SampleEvent>>, StoreError> {
        self.inner
            .read_from_within(transaction, aggregate, kind, from)
            .await
    }

    async fn read_through(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
        through: Version,
    ) -> Result<Vec<Recorded<SampleEvent>>, StoreError> {
        self.inner
            .read_through(aggregate, kind, from, through)
            .await
    }

    async fn latest_snapshot(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<Option<Recorded<SampleEvent>>, StoreError> {
        self.inner.latest_snapshot(aggregate, kind).await
    }

    async fn latest_snapshot_within(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<Option<Recorded<SampleEvent>>, StoreError> {
        self.inner
            .latest_snapshot_within(transaction, aggregate, kind)
            .await
    }

    async fn snapshot_at_or_before(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        version: Version,
    ) -> Result<Option<Recorded<SampleEvent>>, StoreError> {
        self.inner
            .snapshot_at_or_before(aggregate, kind, version)
            .await
    }

    async fn prune_through(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        through: Version,
    ) -> Result<(), StoreError> {
        self.inner
            .prune_through(transaction, aggregate, kind, through)
            .await
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<(), StoreError> {
        self.inner.forget(transaction, aggregate, kind).await
    }
}

struct RefusesSnapshots {
    inner: Arc<InMemoryEventStore<SampleEvent>>,
    outdated: bool,
    refused: Mutex<u8>,
}

impl RefusesSnapshots {
    fn as_outdated(inner: Arc<InMemoryEventStore<SampleEvent>>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            outdated: true,
            refused: Mutex::new(0),
        })
    }

    fn as_broken(inner: Arc<InMemoryEventStore<SampleEvent>>) -> Arc<Self> {
        Arc::new(Self {
            inner,
            outdated: false,
            refused: Mutex::new(0),
        })
    }

    fn refused(&self) -> u8 {
        *self.refused.lock().expect("nothing panics holding this")
    }
}

#[async_trait::async_trait]
impl EventStore<SampleEvent> for RefusesSnapshots {
    async fn append(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        expected: Version,
        stream: &[Recorded<SampleEvent>],
    ) -> Result<(), StoreError> {
        if stream.iter().any(|entry| entry.metadata.is_snapshot) {
            *self.refused.lock().expect("nothing panics holding this") += 1;
            return Err(if self.outdated {
                StoreError::outdated(aggregate, kind, expected)
            } else {
                StoreError::backend(aggregate, kind, "the disk is full")
            });
        }

        self.inner
            .append(transaction, aggregate, kind, expected, stream)
            .await
    }

    async fn read_from(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
    ) -> Result<Vec<Recorded<SampleEvent>>, StoreError> {
        self.inner.read_from(aggregate, kind, from).await
    }

    async fn read_from_within(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
    ) -> Result<Vec<Recorded<SampleEvent>>, StoreError> {
        self.inner
            .read_from_within(transaction, aggregate, kind, from)
            .await
    }

    async fn read_through(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
        through: Version,
    ) -> Result<Vec<Recorded<SampleEvent>>, StoreError> {
        self.inner
            .read_through(aggregate, kind, from, through)
            .await
    }

    async fn latest_snapshot(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<Option<Recorded<SampleEvent>>, StoreError> {
        self.inner.latest_snapshot(aggregate, kind).await
    }

    async fn latest_snapshot_within(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<Option<Recorded<SampleEvent>>, StoreError> {
        self.inner
            .latest_snapshot_within(transaction, aggregate, kind)
            .await
    }

    async fn snapshot_at_or_before(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        version: Version,
    ) -> Result<Option<Recorded<SampleEvent>>, StoreError> {
        self.inner
            .snapshot_at_or_before(aggregate, kind, version)
            .await
    }

    async fn prune_through(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        through: Version,
    ) -> Result<(), StoreError> {
        self.inner
            .prune_through(transaction, aggregate, kind, through)
            .await
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<(), StoreError> {
        self.inner.forget(transaction, aggregate, kind).await
    }
}

async fn a_sample_one_short_of_a_snapshot(
    store: Arc<InMemoryEventStore<SampleEvent>>,
) -> AggregateId {
    let service = a_service(store);
    let id = a_created_sample(&service).await;
    for counted in 2..100 {
        service
            .execute_committed(
                &id,
                a_retitling(&format!("Title {counted}")),
                &crate::an_author(),
            )
            .await
            .expect("updating should succeed");
    }

    id
}

fn a_service_over(store: Arc<RefusesSnapshots>) -> EventSourcingService<Sample> {
    EventSourcingService::new(store, Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)))
}

fn titled_100(store_read: Vec<Recorded<SampleEvent>>) -> usize {
    store_read
        .into_iter()
        .filter(|entry| entry.event == SampleEvent::TitleUpdated("Title 100".to_owned()))
        .count()
}

#[tokio::test]
async fn a_command_whose_snapshot_fails_lands_neither() {
    // given: a sample one event short of a snapshot
    let store = a_store();
    let id = a_sample_one_short_of_a_snapshot(store.clone()).await;

    // and: a store that breaks on snapshots
    let broken = RefusesSnapshots::as_broken(store.clone());

    // when: the event that makes it due is executed
    let refused = a_service_over(broken.clone())
        .execute_committed(&id, a_retitling("Title 100"), &crate::an_author())
        .await;

    // then: it fails as the backend's failure
    assert!(matches!(
        refused,
        Err(ServiceError::Store(StoreError::Backend { .. }))
    ));
    // and: is not tried again
    assert_eq!(
        broken.refused(),
        1,
        "a store that broke did not lose a race, so the command is not tried again"
    );
    // and: the event did not land
    assert_eq!(
        titled_100(
            store
                .read_from(&id, Sample::KIND, Version::ZERO)
                .await
                .expect("reading should succeed")
        ),
        0,
        "the command and its snapshot share a transaction, so the command is rolled back with it, and tried again is never appended twice"
    );
}

#[tokio::test]
async fn a_command_whose_snapshot_lost_a_race_is_tried_again_whole() {
    // given: a sample one event short of a snapshot
    let store = a_store();
    let id = a_sample_one_short_of_a_snapshot(store.clone()).await;

    // and: a store where every snapshot loses a race
    let overtaken = RefusesSnapshots::as_outdated(store.clone());

    // when: the event that makes it due is executed
    let refused = a_service_over(overtaken.clone())
        .execute_committed(&id, a_retitling("Title 100"), &crate::an_author())
        .await;

    // then: it is refused as outdated
    assert!(matches!(
        refused,
        Err(ServiceError::Store(StoreError::Outdated { .. }))
    ));
    // and: the event did not land
    assert_eq!(
        titled_100(
            store
                .read_from(&id, Sample::KIND, Version::ZERO)
                .await
                .expect("reading should succeed")
        ),
        0,
        "each try rolled back whole, the command with its snapshot"
    );
    // and: it was tried as often as tries allow
    assert_eq!(overtaken.refused(), transacting::retrying::TRIES);
}

#[tokio::test]
async fn a_command_executed_within_a_transaction_takes_its_snapshot_in_it() {
    // given: a sample one event short of a snapshot
    let store = a_store();
    let id = a_sample_one_short_of_a_snapshot(store.clone()).await;
    // and: a transaction
    let service = a_service(store.clone());
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");

    // when: the event that makes it due is executed within it, which is then finished
    let landed = service
        .execute(
            &mut transaction,
            &id,
            a_retitling("Title 100"),
            &crate::an_author(),
        )
        .await
        .expect("updating should succeed");
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    // then: the snapshot's version is answered
    assert_eq!(
        landed.version,
        Version::of(101),
        "the version answered is the snapshot's, so whoever aims at it next is not refused"
    );
    // and: the snapshot is stored
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
}

#[tokio::test]
async fn a_snapshot_taken_within_a_transaction_that_is_dropped_was_never_taken() {
    // given: a sample one event short of a snapshot
    let store = a_store();
    let id = a_sample_one_short_of_a_snapshot(store.clone()).await;
    // and: a transaction that executed the event making it due
    let service = a_service(store.clone());
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    service
        .execute(
            &mut transaction,
            &id,
            a_retitling("Title 100"),
            &crate::an_author(),
        )
        .await
        .expect("updating should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: no snapshot is stored
    assert!(
        store
            .latest_snapshot(&id, Sample::KIND)
            .await
            .expect("looking should succeed")
            .is_none()
    );
}

#[tokio::test]
async fn a_command_aimed_at_a_version_takes_the_snapshot_it_makes_due() {
    // given: a sample one event short of a snapshot
    let store = a_store();
    let id = a_sample_one_short_of_a_snapshot(store.clone()).await;

    // when: the event that makes it due is executed at its version
    let landed = a_service(store.clone())
        .execute_at_committed(
            &id,
            Version::of(99),
            a_retitling("Title 100"),
            &crate::an_author(),
        )
        .await
        .expect("updating should succeed");

    // then: the snapshot's version is answered
    assert_eq!(landed.version, Version::of(101));
    // and: the snapshot is stored
    assert_eq!(
        store
            .latest_snapshot(&id, Sample::KIND)
            .await
            .expect("looking should succeed")
            .expect("a snapshot was due")
            .metadata
            .version,
        Version::of(101),
        "a command aimed at a version, handed no transaction, commits the one it opened"
    );
}

async fn executing_within(
    service: &EventSourcingService<Sample>,
    command: SampleCommand,
) -> (Transaction, Outcome<Sample>) {
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");
    let executed = service
        .execute(
            &mut transaction,
            &sample::a_sample(),
            command,
            &crate::an_author(),
        )
        .await;

    (transaction, executed)
}

async fn a_sample_begun(service: &EventSourcingService<Sample>) {
    service
        .begin_committed(&sample::a_sample(), a_creation(), &crate::an_author())
        .await
        .expect("the sample is created");
}

#[tokio::test]
async fn a_command_executed_within_a_transaction_lands_when_its_owner_finishes_it() {
    // given: a created sample
    let service = a_workbench();
    a_sample_begun(&service).await;

    // when: it is retitled within a transaction its owner finishes
    let (transaction, executed) = executing_within(&service, a_retitling("Rewoven")).await;
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    // then: the next version landed
    assert_eq!(
        executed.expect("executing should succeed").version,
        Version::of(2)
    );
    // and: it reads retitled
    assert_eq!(
        service
            .latest(&sample::a_sample())
            .await
            .expect("it was begun")
            .state
            .title,
        "Rewoven"
    );
}

#[tokio::test]
async fn a_command_executed_within_a_transaction_that_is_dropped_never_landed() {
    // given: a created sample
    let service = a_workbench();
    a_sample_begun(&service).await;

    // when: it is retitled within a transaction that is dropped
    let (transaction, executed) = executing_within(&service, a_retitling("Rewoven")).await;
    executed.expect("executing should succeed");
    drop(transaction);

    // then: it reads as it was
    let standing = service
        .latest(&sample::a_sample())
        .await
        .expect("it was begun");
    assert_eq!(
        (standing.state.title.as_str(), standing.version),
        ("The Loom", Version::of(1))
    );
}

#[tokio::test]
async fn a_command_executed_within_a_transaction_is_not_tried_again_but_left_to_its_owner() {
    // given: a created sample
    let store = a_store();
    a_sample_begun(&a_service(store.clone())).await;
    // and: a store refusing the next append as outdated
    let balking = BalksOnce::wrapping(store, 1);
    let service = EventSourcingService::<Sample>::new(
        balking.clone(),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );

    // when: it is retitled within a transaction
    let (_, executed) = executing_within(&service, a_retitling("Rewoven")).await;

    // then: it is refused as outdated
    assert!(
        matches!(
            executed,
            Err(ServiceError::Store(StoreError::Outdated { .. }))
        ),
        "only the owner of the transaction can try the whole of it again"
    );
}

#[tokio::test]
async fn a_command_executed_within_a_transaction_on_nothing_is_not_found() {
    // given: nothing created
    let service = a_workbench();

    // when: a sample is retitled within a transaction
    let (_, executed) = executing_within(&service, a_retitling("Rewoven")).await;

    // then: it is not found
    assert!(matches!(executed, Err(ServiceError::NotFound { .. })));
}

#[tokio::test]
async fn a_command_executed_within_a_transaction_that_changes_nothing_appends_nothing() {
    // given: a shelved sample
    let service = a_workbench();
    a_sample_begun(&service).await;
    let shelved = || SampleCommand::Shelve("shelf_1".to_owned());
    service
        .execute_committed(&sample::a_sample(), shelved(), &crate::an_author())
        .await
        .expect("shelving should succeed");

    // when: it is shelved there again within a transaction its owner finishes
    let (transaction, executed) = executing_within(&service, shelved()).await;
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    // then: nothing changed
    let executed = executed.expect("executing should succeed");
    assert!(executed.changed_nothing());
    // and: its version stays
    assert_eq!(executed.version, Version::of(2));
}

#[tokio::test]
async fn a_command_is_tried_again_when_the_aggregate_moved_on_underneath_it() {
    // given: a created sample
    let store = a_store();
    a_service(store.clone())
        .begin_committed(&sample::a_sample(), a_creation(), &crate::an_author())
        .await
        .expect("the sample is created");
    // and: a store refusing the next two appends as outdated
    let balking = BalksOnce::wrapping(store, 2);
    let service = EventSourcingService::<Sample>::new(
        balking.clone(),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );

    // when: it is retitled
    let landed = service
        .execute_committed(
            &sample::a_sample(),
            SampleCommand::UpdateTitle("Rewoven".to_owned()),
            &crate::an_author(),
        )
        .await
        .expect("two refusals are ridden out");

    // then: the retitling lands once
    assert_eq!(
        events_of(&landed),
        vec![SampleEvent::TitleUpdated("Rewoven".to_owned())]
    );
}

#[tokio::test]
async fn a_command_gives_up_once_it_has_run_out_of_tries() {
    // given: a created sample
    let store = a_store();
    a_service(store.clone())
        .begin_committed(&sample::a_sample(), a_creation(), &crate::an_author())
        .await
        .expect("the sample is created");
    // and: a store refusing every try as outdated
    let service = EventSourcingService::<Sample>::new(
        BalksOnce::wrapping(store, transacting::retrying::TRIES),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );

    // when: it is retitled
    let refused = service
        .execute_committed(
            &sample::a_sample(),
            SampleCommand::UpdateTitle("Rewoven".to_owned()),
            &crate::an_author(),
        )
        .await;

    // then: it is refused as outdated
    assert!(matches!(
        refused,
        Err(ServiceError::Store(StoreError::Outdated { .. }))
    ));
}

#[tokio::test]
async fn a_command_aimed_at_one_version_is_never_tried_again() {
    // given: a created sample
    let store = a_store();
    a_service(store.clone())
        .begin_committed(&sample::a_sample(), a_creation(), &crate::an_author())
        .await
        .expect("the sample is created");
    // and: a store refusing the next append as outdated
    let service = EventSourcingService::<Sample>::new(
        BalksOnce::wrapping(store, 1),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );

    // when: it is retitled at its version
    let refused = service
        .execute_at_committed(
            &sample::a_sample(),
            Version::of(1),
            SampleCommand::UpdateTitle("Rewoven".to_owned()),
            &crate::an_author(),
        )
        .await;

    // then: it is refused as outdated
    assert!(
        matches!(
            refused,
            Err(ServiceError::Store(StoreError::Outdated { .. }))
        ),
        "a caller who named a version must hear that it moved on, not have it papered over"
    );
}

struct Yielding {
    inner: Arc<InMemoryEventStore<SampleEvent>>,
}

#[async_trait::async_trait]
impl EventStore<SampleEvent> for Yielding {
    async fn append(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        expected: Version,
        stream: &[Recorded<SampleEvent>],
    ) -> Result<(), StoreError> {
        self.inner
            .append(transaction, aggregate, kind, expected, stream)
            .await
    }

    async fn read_from(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
    ) -> Result<Vec<Recorded<SampleEvent>>, StoreError> {
        let read = self.inner.read_from(aggregate, kind, from).await;
        tokio::task::yield_now().await;

        read
    }

    async fn read_from_within(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
    ) -> Result<Vec<Recorded<SampleEvent>>, StoreError> {
        let read = self
            .inner
            .read_from_within(transaction, aggregate, kind, from)
            .await;
        tokio::task::yield_now().await;

        read
    }

    async fn read_through(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        from: Version,
        through: Version,
    ) -> Result<Vec<Recorded<SampleEvent>>, StoreError> {
        self.inner
            .read_through(aggregate, kind, from, through)
            .await
    }

    async fn latest_snapshot(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<Option<Recorded<SampleEvent>>, StoreError> {
        self.inner.latest_snapshot(aggregate, kind).await
    }

    async fn latest_snapshot_within(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<Option<Recorded<SampleEvent>>, StoreError> {
        self.inner
            .latest_snapshot_within(transaction, aggregate, kind)
            .await
    }

    async fn snapshot_at_or_before(
        &self,
        aggregate: &AggregateId,
        kind: AggregateType,
        version: Version,
    ) -> Result<Option<Recorded<SampleEvent>>, StoreError> {
        self.inner
            .snapshot_at_or_before(aggregate, kind, version)
            .await
    }

    async fn prune_through(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
        through: Version,
    ) -> Result<(), StoreError> {
        self.inner
            .prune_through(transaction, aggregate, kind, through)
            .await
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        aggregate: &AggregateId,
        kind: AggregateType,
    ) -> Result<(), StoreError> {
        self.inner.forget(transaction, aggregate, kind).await
    }
}

#[tokio::test(flavor = "current_thread")]
async fn many_commands_on_one_aggregate_at_once_all_land_however_often_they_collide() {
    // given: a created sample
    let store = a_store();
    a_service(store.clone())
        .begin_committed(&sample::a_sample(), a_creation(), &crate::an_author())
        .await
        .expect("the sample is created");
    // and: a store that yields while reading
    let service = Arc::new(EventSourcingService::<Sample>::new(
        Arc::new(Yielding { inner: store }),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    ));

    // when: eight retitlings run at once, each retried
    let transactions = Arc::new(InMemoryTransactions::new());

    let mut commanded = tokio::task::JoinSet::new();
    for nth in 0..8 {
        let (service, transactions) = (service.clone(), transactions.clone());
        commanded.spawn(async move {
            let landed: Outcome<Sample> = transacting::retrying!(&*transactions, |transaction| {
                service
                    .execute(
                        transaction,
                        &sample::a_sample(),
                        SampleCommand::UpdateTitle(format!("Rewoven {nth}")),
                        &crate::an_author(),
                    )
                    .await
            });
            landed.map(|landed| landed.version)
        });
    }
    let mut reached = Vec::new();
    while let Some(landed) = commanded.join_next().await {
        reached.push(
            landed
                .expect("the command runs to its end")
                .expect("a collision with a command at the same time is ridden out, not refused"),
        );
    }
    reached.sort();

    // then: each lands a version of its own
    assert_eq!(
        reached,
        (2..=9).map(Version::of).collect::<Vec<_>>(),
        "eight writers of one feature, retried by the core as it owns their transactions, each land however often they collide"
    );
}

fn shelves_in(stream: &[Recorded<SampleEvent>]) -> Vec<Option<String>> {
    stream
        .iter()
        .map(|entry| entry.metadata.context.get("shelf").map(str::to_owned))
        .collect()
}

#[tokio::test]
async fn the_events_of_a_command_carry_the_context_the_command_left_the_aggregate_in() {
    // given: nothing created
    let store = a_store();
    let service = a_service(store.clone());

    // when: a sample is created and then shelved
    let created = service
        .begin_committed(&sample::a_sample(), a_creation(), &crate::an_author())
        .await
        .expect("the sample is created");
    let shelved = service
        .execute_committed(
            &sample::a_sample(),
            SampleCommand::Shelve("shelf_1".to_owned()),
            &crate::an_author(),
        )
        .await
        .expect("the sample is shelved");

    // then: the creation's event is in no context
    assert_eq!(
        shelves_in(&created.events),
        [None],
        "a sample on no shelf is in no context"
    );
    // and: the shelving's is in the shelf's
    assert_eq!(
        shelves_in(&shelved.events),
        [Some("shelf_1".to_owned())],
        "the context is the aggregate's after the command, not before it"
    );
    // and: they are stored so
    let stored = store
        .read_from(&sample::a_sample(), sample::SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");
    assert_eq!(shelves_in(&stored), [None, Some("shelf_1".to_owned())]);
}

#[tokio::test]
async fn a_snapshot_carries_the_context_of_what_it_holds_and_what_follows_it_keeps_it() {
    // given: a shelved sample
    let store = a_store();
    let service = a_service(store.clone());
    service
        .begin_committed(&sample::a_sample(), a_creation(), &crate::an_author())
        .await
        .expect("the sample is created");
    service
        .execute_committed(
            &sample::a_sample(),
            SampleCommand::Shelve("shelf_1".to_owned()),
            &crate::an_author(),
        )
        .await
        .expect("the sample is shelved");

    // when: it is collapsed and then retitled
    service
        .compact_committed(&sample::a_sample(), &crate::an_author())
        .await
        .expect("compacting should succeed");
    let retitled = service
        .execute_committed(
            &sample::a_sample(),
            SampleCommand::UpdateTitle("Rewoven".to_owned()),
            &crate::an_author(),
        )
        .await
        .expect("the sample is retitled");

    // then: the snapshot is in the shelf's context
    let snapshot = store
        .latest_snapshot(&sample::a_sample(), sample::SAMPLE)
        .await
        .expect("reading should succeed")
        .expect("the snapshot is there");
    assert_eq!(snapshot.metadata.context.get("shelf"), Some("shelf_1"));
    // and: so is the retitling's event
    assert_eq!(
        shelves_in(&retitled.events),
        [Some("shelf_1".to_owned())],
        "grown from the snapshot, the sample still knows its shelf"
    );
}

#[test]
fn a_write_aimed_at_a_version_is_tried_once_and_one_aimed_at_none_until_it_stops_losing_races() {
    // expect: a write aimed at a version is tried once, one aimed at none as often as tries allow
    assert_eq!(
        (tries_for(Some(Version::of(3))), tries_for(None)),
        (1, transacting::retrying::TRIES),
        "a conflict with the version named is the answer, a 412, never papered over"
    );
}
