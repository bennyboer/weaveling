use crate::aggregate::AggregateId;
use crate::codec::Codec;
use crate::context::AggregateContext;
use crate::event::{Event, Recorded};
use crate::store::{EventStore, StoreError};
use crate::testing::sample;
use crate::testing::sample::{SAMPLE, SampleEvent, SampleKind};
use crate::testing::{Committing, Tables, Workbench};
use crate::version::Version;

fn a_creation() -> SampleEvent {
    SampleEvent::Created {
        title: "The Loom".to_owned(),
        description: "A silent machine.".to_owned(),
        kind: SampleKind::Ordinary,
    }
}

fn a_snapshot() -> SampleEvent {
    SampleEvent::Snapshotted {
        title: "The Loom".to_owned(),
        description: "A silent machine.".to_owned(),
        deleted: false,
        shelf: None,
    }
}

fn at(aggregate: &AggregateId, version: u64, event: SampleEvent) -> Recorded<SampleEvent> {
    let mut metadata = sample::stamped(aggregate, version);
    metadata.is_snapshot = matches!(event, SampleEvent::Snapshotted { .. });

    Recorded { event, metadata }
}

async fn a_stream(
    store: &Committing<'_, impl EventStore<SampleEvent>>,
    aggregate: &AggregateId,
    events: Vec<SampleEvent>,
) {
    let stream = sample::recorded(aggregate, events);

    for (counted, entry) in stream.into_iter().enumerate() {
        store
            .append(
                aggregate,
                SAMPLE,
                Version::of(counted as u64),
                std::slice::from_ref(&entry),
            )
            .await
            .expect("appending a fresh event should succeed");
    }
}

pub async fn an_empty_stream_is_at_version_zero(
    store: &Committing<'_, impl EventStore<SampleEvent>>,
) {
    // given: an aggregate nothing was appended for
    let aggregate = AggregateId::from("sample_empty");

    // when: its stream is read
    let found = store
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await
        .expect("reading an unknown stream should not fail");

    // then: it is empty
    assert!(
        found.is_empty(),
        "a stream nobody has written to must read as empty, not as missing"
    );
}

pub async fn appended_events_read_back_in_order(
    store: &Committing<'_, impl EventStore<SampleEvent>>,
) {
    // given: a stream of three events
    let aggregate = AggregateId::from("sample_ordered");
    a_stream(
        store,
        &aggregate,
        vec![
            a_creation(),
            SampleEvent::TitleUpdated("The Silent Loom".to_owned()),
            SampleEvent::DescriptionUpdated("It remembers.".to_owned()),
        ],
    )
    .await;

    // when: it is read
    let found = store
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");

    // then: the three come back in order
    assert_eq!(found.len(), 3);
    assert_eq!(found[0].event, a_creation());
    assert_eq!(
        found[2].event,
        SampleEvent::DescriptionUpdated("It remembers.".to_owned())
    );
}

pub async fn appending_at_a_stale_version_is_refused(
    store: &Committing<'_, impl EventStore<SampleEvent>>,
) {
    // given: a stream of one event
    let aggregate = AggregateId::from("sample_stale");
    a_stream(store, &aggregate, vec![a_creation()]).await;

    // when: an event is appended as if it were empty
    let refused = store
        .append(
            &aggregate,
            SAMPLE,
            Version::ZERO,
            &[at(
                &aggregate,
                1,
                SampleEvent::TitleUpdated("Too late".to_owned()),
            )],
        )
        .await;

    // then: the append is refused as outdated
    assert_eq!(
        refused,
        Err(StoreError::Outdated {
            aggregate: aggregate.clone(),
            kind: SAMPLE,
            expected: Version::ZERO,
        })
    );
}

pub async fn a_refused_append_leaves_the_stream_untouched(
    store: &Committing<'_, impl EventStore<SampleEvent>>,
) {
    // given: a stream of one event
    let aggregate = AggregateId::from("sample_untouched");
    a_stream(store, &aggregate, vec![a_creation()]).await;

    // when: an append at a version it never reached is made
    let _ = store
        .append(
            &aggregate,
            SAMPLE,
            Version::of(7),
            &[at(&aggregate, 8, SampleEvent::Deleted)],
        )
        .await;

    // then: the stream still holds its one event
    let found = store
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");

    assert_eq!(
        found.len(),
        1,
        "a rejected append must not write any of its events"
    );
}

pub async fn a_pruned_stream_cannot_be_started_over(
    store: &Committing<'_, impl EventStore<SampleEvent>>,
) {
    // given: a stream of a creation, a deletion and a snapshot
    let aggregate = AggregateId::from("sample_looks_empty");
    a_stream(
        store,
        &aggregate,
        vec![a_creation(), SampleEvent::Deleted, a_snapshot()],
    )
    .await;

    // and: pruned up to the snapshot
    store
        .prune_through(&aggregate, SAMPLE, Version::of(2))
        .await
        .expect("pruning should succeed");

    // when: it is begun anew
    let refused = store
        .append(
            &aggregate,
            SAMPLE,
            Version::ZERO,
            &[at(&aggregate, 1, a_creation())],
        )
        .await;

    // then: that is refused as outdated
    assert!(
        matches!(refused, Err(StoreError::Outdated { .. })),
        "a stream collapsed to a snapshot has a history, however little of it is left: {refused:?}"
    );

    // and: only the snapshot is left
    let found = store
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");

    assert_eq!(
        found.len(),
        1,
        "starting over would have written a second version 1 under the snapshot"
    );
}

pub async fn a_forgotten_stream_is_gone_whole_and_the_others_stay(
    store: &Committing<'_, impl EventStore<SampleEvent>>,
) {
    // given: a stream ending in a snapshot
    let forgotten = AggregateId::from("sample_forgotten");
    let kept = AggregateId::from("sample_kept");
    a_stream(
        store,
        &forgotten,
        vec![a_creation(), SampleEvent::Deleted, a_snapshot()],
    )
    .await;
    // and: another stream
    a_stream(store, &kept, vec![a_creation()]).await;

    // when: the first is forgotten, twice
    store
        .forget(&forgotten, SAMPLE)
        .await
        .expect("forgetting should succeed");
    store
        .forget(&forgotten, SAMPLE)
        .await
        .expect("forgetting what is gone already changes nothing");

    // then: none of its events are left
    assert!(
        store
            .read_from(&forgotten, SAMPLE, Version::ZERO)
            .await
            .expect("reading should succeed")
            .is_empty()
    );
    // and: nor its snapshot
    assert!(
        store
            .latest_snapshot(&forgotten, SAMPLE)
            .await
            .expect("reading should succeed")
            .is_none(),
        "the snapshot goes with it, unlike pruning"
    );
    // and: the other stream stays
    assert_eq!(
        store
            .read_from(&kept, SAMPLE, Version::ZERO)
            .await
            .expect("reading should succeed")
            .len(),
        1
    );
    // and: the forgotten one can be begun anew
    store
        .append(
            &forgotten,
            SAMPLE,
            Version::ZERO,
            &[at(&forgotten, 1, a_creation())],
        )
        .await
        .expect("a stream forgotten whole has no history left to collide with");
}

pub async fn one_append_may_carry_several_events(
    store: &Committing<'_, impl EventStore<SampleEvent>>,
) {
    // when: two events are appended at once
    let aggregate = AggregateId::from("sample_batch");

    store
        .append(
            &aggregate,
            SAMPLE,
            Version::ZERO,
            &[
                at(&aggregate, 1, a_creation()),
                at(
                    &aggregate,
                    2,
                    SampleEvent::TitleUpdated("The Silent Loom".to_owned()),
                ),
            ],
        )
        .await
        .expect("appending a batch should succeed");

    // then: both are stored
    let found = store
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");

    assert_eq!(found.len(), 2, "a batch must be stored whole");
}

pub async fn streams_of_different_aggregates_do_not_mix(
    store: &Committing<'_, impl EventStore<SampleEvent>>,
) {
    // given: a stream of one event and another of two
    let one = AggregateId::from("sample_one");
    let other = AggregateId::from("sample_other");
    a_stream(store, &one, vec![a_creation()]).await;
    a_stream(
        store,
        &other,
        vec![a_creation(), SampleEvent::TitleUpdated("Apart".to_owned())],
    )
    .await;

    // when: each is read
    let first = store
        .read_from(&one, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");
    let second = store
        .read_from(&other, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");

    // then: each holds only its own
    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 2);
}

pub async fn reading_from_a_version_skips_what_came_before(
    store: &Committing<'_, impl EventStore<SampleEvent>>,
) {
    // given: a stream of three events
    let aggregate = AggregateId::from("sample_from");
    a_stream(
        store,
        &aggregate,
        vec![
            a_creation(),
            SampleEvent::TitleUpdated("Second".to_owned()),
            SampleEvent::TitleUpdated("Third".to_owned()),
        ],
    )
    .await;

    // when: it is read from version two
    let found = store
        .read_from(&aggregate, SAMPLE, Version::of(2))
        .await
        .expect("reading should succeed");

    // then: versions two and three are read
    assert_eq!(
        found.len(),
        2,
        "reading from a version includes that version"
    );
    assert_eq!(found[0].metadata.version, Version::of(2));
}

pub async fn reading_through_a_version_stops_there(
    store: &Committing<'_, impl EventStore<SampleEvent>>,
) {
    // given: a stream of three events
    let aggregate = AggregateId::from("sample_through");
    a_stream(
        store,
        &aggregate,
        vec![
            a_creation(),
            SampleEvent::TitleUpdated("Second".to_owned()),
            SampleEvent::TitleUpdated("Third".to_owned()),
        ],
    )
    .await;

    // when: it is read through version two
    let found = store
        .read_through(&aggregate, SAMPLE, Version::ZERO, Version::of(2))
        .await
        .expect("reading should succeed");

    // then: versions one and two are read
    assert_eq!(found.len(), 2, "reading through a version includes it");
    assert_eq!(found[1].metadata.version, Version::of(2));
}

pub async fn a_stream_without_snapshots_has_none(
    store: &Committing<'_, impl EventStore<SampleEvent>>,
) {
    // given: a stream without snapshots
    let aggregate = AggregateId::from("sample_nosnap");
    a_stream(store, &aggregate, vec![a_creation()]).await;

    // when: its latest snapshot is looked for
    let found = store
        .latest_snapshot(&aggregate, SAMPLE)
        .await
        .expect("looking for a snapshot should not fail");

    // then: there is none
    assert!(found.is_none());
}

pub async fn the_latest_snapshot_is_the_newest_one(
    store: &Committing<'_, impl EventStore<SampleEvent>>,
) {
    // given: a stream with two snapshots
    let aggregate = AggregateId::from("sample_latest");
    a_stream(
        store,
        &aggregate,
        vec![
            a_creation(),
            a_snapshot(),
            SampleEvent::TitleUpdated("After".to_owned()),
            a_snapshot(),
        ],
    )
    .await;

    // when: its latest snapshot is looked for
    let found = store
        .latest_snapshot(&aggregate, SAMPLE)
        .await
        .expect("looking for a snapshot should not fail")
        .expect("this stream has snapshots");

    // then: it is the newer one
    assert_eq!(found.metadata.version, Version::of(4));
}

pub async fn a_snapshot_can_be_found_as_of_an_older_version(
    store: &Committing<'_, impl EventStore<SampleEvent>>,
) {
    // given: a stream with snapshots at versions two and four
    let aggregate = AggregateId::from("sample_asof");
    a_stream(
        store,
        &aggregate,
        vec![
            a_creation(),
            a_snapshot(),
            SampleEvent::TitleUpdated("After".to_owned()),
            a_snapshot(),
        ],
    )
    .await;

    // when: a snapshot as of version three is looked for
    let found = store
        .snapshot_at_or_before(&aggregate, SAMPLE, Version::of(3))
        .await
        .expect("looking for a snapshot should not fail")
        .expect("there is a snapshot at or before version 3");

    // then: it is the one at version two
    assert_eq!(
        found.metadata.version,
        Version::of(2),
        "time travel must not see a snapshot from the future"
    );
}

pub async fn pruning_discards_events_a_snapshot_replaced(
    store: &Committing<'_, impl EventStore<SampleEvent>>,
) {
    // given: a stream whose third event is a snapshot
    let aggregate = AggregateId::from("sample_pruned");
    a_stream(
        store,
        &aggregate,
        vec![
            a_creation(),
            SampleEvent::TitleUpdated("Second".to_owned()),
            a_snapshot(),
        ],
    )
    .await;

    // when: it is pruned up to the snapshot
    store
        .prune_through(&aggregate, SAMPLE, Version::of(2))
        .await
        .expect("pruning should succeed");

    // then: only the snapshot is left
    let found = store
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].metadata.version, Version::of(3));
}

pub async fn appending_after_pruning_continues_the_version_count(
    store: &Committing<'_, impl EventStore<SampleEvent>>,
) {
    // given: a stream of two events
    let aggregate = AggregateId::from("sample_after_prune");
    a_stream(
        store,
        &aggregate,
        vec![a_creation(), SampleEvent::TitleUpdated("Second".to_owned())],
    )
    .await;

    // and: pruned of the first
    store
        .prune_through(&aggregate, SAMPLE, Version::of(1))
        .await
        .expect("pruning should succeed");

    // when: an event is appended after the second
    store
        .append(
            &aggregate,
            SAMPLE,
            Version::of(2),
            &[at(&aggregate, 3, SampleEvent::Deleted)],
        )
        .await
        .expect("a pruned stream must remember how far it got");

    // then: it is at version three
    let found = store
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");

    assert_eq!(
        found.last().expect("not empty").metadata.version,
        Version::of(3)
    );
}

fn retitled(title: &str) -> SampleEvent {
    SampleEvent::TitleUpdated(title.to_owned())
}

async fn titles_in(
    store: &Committing<'_, impl EventStore<SampleEvent>>,
    aggregate: &AggregateId,
) -> Vec<String> {
    store
        .read_from(aggregate, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed")
        .into_iter()
        .filter_map(|happened| match happened.event {
            SampleEvent::TitleUpdated(title) => Some(title),
            _ => None,
        })
        .collect()
}

pub async fn a_read_within_a_transaction_sees_what_it_appended(bench: &impl Workbench) {
    // given: a transaction that appended a creation and a snapshot
    let aggregate = AggregateId::from("sample_within");
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    bench
        .store()
        .append(
            &mut transaction,
            &aggregate,
            SAMPLE,
            Version::ZERO,
            &sample::recorded(&aggregate, vec![a_creation(), a_snapshot()]),
        )
        .await
        .expect("appending should succeed");

    // when: the stream and its latest snapshot are read within it
    let read = bench
        .store()
        .read_from_within(&mut transaction, &aggregate, SAMPLE, Version::ZERO)
        .await
        .expect("reading within should succeed");
    let snapshot = bench
        .store()
        .latest_snapshot_within(&mut transaction, &aggregate, SAMPLE)
        .await
        .expect("reading within should succeed");
    // and: the transaction is dropped
    drop(transaction);

    // then: the read saw both events
    assert_eq!(
        read.len(),
        2,
        "a command reads what its own transaction appended, or a second command on the aggregate is outdated"
    );
    // and: the snapshot
    assert_eq!(
        snapshot.map(|snapshot| snapshot.metadata.version),
        Some(Version::of(2))
    );
    // and: nothing is left outside the transaction
    assert!(
        bench
            .store()
            .read_from(&aggregate, SAMPLE, Version::ZERO)
            .await
            .expect("reading should succeed")
            .is_empty(),
        "what was read within a transaction dropped unfinished is gone with it"
    );
}

pub async fn an_append_leaves_a_message_waiting_for_every_publishable_event(
    bench: &impl Workbench,
) {
    // when: a creation and a retitling are appended
    let aggregate = AggregateId::from("sample_announced");

    a_stream(
        &bench.committing(),
        &aggregate,
        vec![a_creation(), retitled("Second")],
    )
    .await;

    // then: a message waits for each
    assert_eq!(
        bench.waiting().await,
        ["sample.created", "sample.title_updated"],
        "an append enqueues, it does not publish"
    );
}

pub async fn an_event_that_publishes_nothing_leaves_nothing_waiting(bench: &impl Workbench) {
    // when: a creation, a correction and a snapshot are appended
    let aggregate = AggregateId::from("sample_quiet");

    a_stream(
        &bench.committing(),
        &aggregate,
        vec![
            a_creation(),
            SampleEvent::Corrected("Quietly".to_owned()),
            a_snapshot(),
        ],
    )
    .await;

    // then: a message waits for the creation alone
    assert_eq!(
        bench.waiting().await,
        ["sample.created"],
        "a correction and a snapshot are both unpublishable, so only the creation waits"
    );
}

pub async fn a_refused_append_leaves_no_message_waiting(bench: &impl Workbench) {
    // when: an append at a version never reached is made
    let aggregate = AggregateId::from("sample_refused");

    let refused = bench
        .committing()
        .append(
            &aggregate,
            SAMPLE,
            Version::of(7),
            &[at(&aggregate, 8, a_creation())],
        )
        .await;

    // then: it is refused
    assert!(refused.is_err());
    // and: no message waits
    assert!(
        bench.waiting().await.is_empty(),
        "the event and its message are written together or not at all"
    );
}

pub async fn a_dropped_append_leaves_neither_its_events_nor_their_messages(bench: &impl Workbench) {
    // given: a transaction that appended a creation
    let aggregate = AggregateId::from("sample_dropped");
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    bench
        .store()
        .append(
            &mut transaction,
            &aggregate,
            SAMPLE,
            Version::ZERO,
            &[at(&aggregate, 1, a_creation())],
        )
        .await
        .expect("appending a fresh event should succeed");

    // when: it is dropped
    drop(transaction);

    // then: the creation is not there
    assert!(
        bench
            .store()
            .read_from(&aggregate, SAMPLE, Version::ZERO)
            .await
            .expect("reading should succeed")
            .is_empty(),
        "an append rolled back is not there"
    );
    // and: no message waits
    assert!(bench.waiting().await.is_empty(), "and nothing announces it");
    // and: the stream can be begun anew
    a_stream(&bench.committing(), &aggregate, vec![a_creation()]).await;
}

pub async fn a_dropped_append_onto_a_stream_leaves_the_stream_as_it_was(bench: &impl Workbench) {
    // given: a stream of one event
    let aggregate = AggregateId::from("sample_dropped_onto");
    a_stream(&bench.committing(), &aggregate, vec![a_creation()]).await;
    // and: a transaction that appended to it
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    bench
        .store()
        .append(
            &mut transaction,
            &aggregate,
            SAMPLE,
            Version::of(1),
            &[at(&aggregate, 2, retitled("Dropped"))],
        )
        .await
        .expect("appending the next event should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: the stream holds only its first event
    assert_eq!(
        bench
            .store()
            .read_from(&aggregate, SAMPLE, Version::ZERO)
            .await
            .expect("reading should succeed")
            .len(),
        1,
        "what was there before the transaction stays, and nothing it appended"
    );
    // and: only the first event's message waits
    assert_eq!(bench.waiting().await, ["sample.created"]);
    // and: the version the dropped append took is free again
    bench
        .committing()
        .append(
            &aggregate,
            SAMPLE,
            Version::of(1),
            &[at(&aggregate, 2, retitled("Kept"))],
        )
        .await
        .expect("the version a dropped append took is free again");
}

pub async fn a_dropped_prune_keeps_what_it_would_have_discarded(bench: &impl Workbench) {
    // given: a stream of three events ending in a snapshot
    let aggregate = AggregateId::from("sample_pruned_back");
    a_stream(
        &bench.committing(),
        &aggregate,
        vec![a_creation(), retitled("Second"), a_snapshot()],
    )
    .await;
    // and: a transaction that pruned it
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    bench
        .store()
        .prune_through(&mut transaction, &aggregate, SAMPLE, Version::of(2))
        .await
        .expect("pruning should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: all three events are kept
    assert_eq!(
        bench
            .store()
            .read_from(&aggregate, SAMPLE, Version::ZERO)
            .await
            .expect("reading should succeed")
            .len(),
        3
    );
}

pub async fn a_dropped_forget_keeps_the_stream(bench: &impl Workbench) {
    // given: a stream of two events
    let aggregate = AggregateId::from("sample_remembered");
    a_stream(
        &bench.committing(),
        &aggregate,
        vec![a_creation(), retitled("Second")],
    )
    .await;
    // and: a transaction that forgot it
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    bench
        .store()
        .forget(&mut transaction, &aggregate, SAMPLE)
        .await
        .expect("forgetting should succeed");

    // when: the transaction is dropped
    drop(transaction);

    // then: both events are kept
    assert_eq!(
        bench
            .store()
            .read_from(&aggregate, SAMPLE, Version::ZERO)
            .await
            .expect("reading should succeed")
            .len(),
        2
    );
}

pub async fn two_writers_at_the_same_version_cannot_both_win(bench: &impl Tables) {
    // given: a stream of one event
    let aggregate = AggregateId::from("sample_contested");
    a_stream(&bench.committing(), &aggregate, vec![a_creation()]).await;
    // and: two batches for its next version
    let batch = |named: &str| {
        vec![
            at(&aggregate, 2, retitled(named)),
            at(
                &aggregate,
                3,
                SampleEvent::DescriptionUpdated(named.to_owned()),
            ),
        ]
    };
    let (mine, yours) = (batch("Mine"), batch("Yours"));

    // when: both are appended at once
    let committing = bench.committing();
    let (mine, yours) = tokio::join!(
        committing.append(&aggregate, SAMPLE, Version::of(1), &mine),
        committing.append(&aggregate, SAMPLE, Version::of(1), &yours),
    );

    // then: exactly one wins
    assert_ne!(
        mine.is_ok(),
        yours.is_ok(),
        "exactly one writer should win, got {mine:?} and {yours:?}"
    );
    // and: the other is refused as outdated
    let refused = mine.err().or(yours.err()).expect("one of them was refused");
    assert!(
        matches!(refused, StoreError::Outdated { .. }),
        "a lost race is a stale version, not a backend failure: {refused:?}"
    );
    // and: only the winner's events are written
    assert_eq!(
        titles_in(&bench.committing(), &aggregate).await.len(),
        1,
        "the loser's events must not be mixed into the winner's"
    );
}

pub async fn a_batch_that_collides_halfway_writes_none_of_itself(bench: &impl Tables) {
    // given: a stream of one event
    let aggregate = AggregateId::from("sample_half");
    a_stream(&bench.committing(), &aggregate, vec![a_creation()]).await;

    // when: a batch whose second event collides with it is appended
    let refused = bench
        .committing()
        .append(
            &aggregate,
            SAMPLE,
            Version::of(1),
            &[
                at(&aggregate, 2, retitled("Second")),
                at(&aggregate, 1, retitled("First again")),
            ],
        )
        .await;

    // then: it is refused as outdated
    assert!(
        matches!(refused, Err(StoreError::Outdated { .. })),
        "a batch colliding with what is already written is a stale append: {refused:?}"
    );
    // and: none of the batch is written
    assert!(
        titles_in(&bench.committing(), &aggregate).await.is_empty(),
        "an append is all of its events or none of them"
    );
    // and: only the stream's first event is told
    assert_eq!(
        bench.waiting().await,
        ["sample.created"],
        "only the stream's start is announced: the outbox row shares its append's transaction"
    );
}

pub async fn a_backend_that_cannot_be_reached_says_so(bench: &impl Tables) {
    // given: a backend that cannot be reached
    let aggregate = AggregateId::from("sample_unreachable");

    bench.close().await;

    // when: it is read and appended to
    let reading = bench
        .store()
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await;
    let appending = bench
        .committing()
        .append(
            &aggregate,
            SAMPLE,
            Version::ZERO,
            &[at(&aggregate, 1, a_creation())],
        )
        .await;
    // then: both fail as the backend's failure
    assert!(
        matches!(reading, Err(StoreError::Backend { .. })),
        "a store that cannot be reached has not gone out of date: {reading:?}"
    );
    assert!(
        matches!(appending, Err(StoreError::Backend { .. })),
        "a store that cannot be reached has not gone out of date: {appending:?}"
    );
}

pub async fn an_event_written_in_a_shape_nothing_can_read_is_a_backend_failure(
    bench: &impl Tables,
) {
    // given: a stream of one event
    let aggregate = AggregateId::from("sample_unreadable");
    a_stream(&bench.committing(), &aggregate, vec![a_creation()]).await;

    // and: an event after it that nothing can read
    bench.write_unreadable_event(&aggregate, 2).await;

    // when: the stream is read
    let found = bench
        .store()
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await;
    // then: it fails as the backend's failure
    assert!(
        matches!(found, Err(StoreError::Backend { .. })),
        "an unreadable row should be reported, not skipped or panicked on: {found:?}"
    );
}

pub async fn what_the_columns_say_matches_what_the_body_says(bench: &impl Tables) {
    // when: a creation is appended
    let aggregate = AggregateId::from("sample_columns");

    a_stream(&bench.committing(), &aggregate, vec![a_creation()]).await;

    // then: the columns say its name, its shape and who did it
    let (name, shape, agent) = bench.columns(&aggregate, 1).await;
    assert_eq!(name, a_creation().name().as_str());
    assert_eq!(shape as u64, a_creation().version().count());
    assert_eq!(agent, "system");
}

pub async fn a_waiting_message_keeps_the_version_of_the_event_it_tells(bench: &impl Tables) {
    // when: a creation and a retitling are appended
    let aggregate = AggregateId::from("sample_versioned");

    a_stream(
        &bench.committing(),
        &aggregate,
        vec![a_creation(), retitled("Second")],
    )
    .await;

    // then: the message waiting for each keeps its event's version
    assert_eq!(bench.versions_waiting().await, [Some(1), Some(2)]);
}

pub async fn a_codec_is_all_a_second_kind_of_event_needs(bench: &impl Tables) {
    // given: a store with a codec of its own
    let store = bench.reading_with(Codec {
        body: |_| serde_json::json!("Deleted"),
        event: |_| Some(SampleEvent::Deleted),
    });
    // when: a creation is appended through it
    let aggregate = AggregateId::from("sample_recoded");

    a_stream(
        &Committing {
            store: &store,
            transactions: bench.transactions(),
        },
        &aggregate,
        vec![a_creation()],
    )
    .await;

    // then: it reads back through that codec
    let found = store
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");
    assert_eq!(
        found[0].event,
        SampleEvent::Deleted,
        "the SQL is written once and the shape on disk belongs to whoever supplied the codec"
    );
}

pub async fn laying_the_schema_down_twice_changes_nothing(bench: &impl Tables) {
    // given: the schema laid down again
    bench.lay_the_schema_down_again().await;

    // then: a stream can still be begun
    a_stream(
        &bench.committing(),
        &AggregateId::from("sample_again"),
        vec![a_creation()],
    )
    .await;
}

pub async fn a_version_below_zero_is_a_backend_failure_not_a_huge_version(bench: &impl Tables) {
    // given: a stream of one event
    let aggregate = AggregateId::from("sample_negative");
    a_stream(&bench.committing(), &aggregate, vec![a_creation()]).await;

    // and: its versions pushed below zero
    bench.push_versions_below_zero(&aggregate).await;

    // when: its latest snapshot is looked for
    let found = bench.committing().latest_snapshot(&aggregate, SAMPLE).await;
    // then: it fails as the backend's failure
    assert!(
        matches!(found, Err(StoreError::Backend { .. })),
        "-3 read as a u64 is eighteen quintillion, a version nothing could ever have written: {found:?}"
    );
}

pub async fn an_event_s_context_is_read_back_as_it_was_written(
    store: &Committing<'_, impl EventStore<SampleEvent>>,
) {
    // given: a creation and a snapshot in a context, and an event in none
    let aggregate = AggregateId::from("sample_in_context");
    let context = AggregateContext::default()
        .with("shelf", "shelf_1")
        .with("room", "room_2");
    let mut created = at(&aggregate, 1, a_creation());
    created.metadata.context = context.clone();
    let mut snapshot = at(&aggregate, 2, a_snapshot());
    snapshot.metadata.context = context.clone();
    let plain = at(&aggregate, 3, SampleEvent::TitleUpdated("Plain".to_owned()));
    // when: they are appended
    for (expected, entry) in [created, snapshot, plain].into_iter().enumerate() {
        store
            .append(
                &aggregate,
                SAMPLE,
                Version::of(expected as u64),
                std::slice::from_ref(&entry),
            )
            .await
            .expect("appending should succeed");
    }

    // then: each reads back in the context it was written in
    let found = store
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");
    let latest = store
        .latest_snapshot(&aggregate, SAMPLE)
        .await
        .expect("reading should succeed")
        .expect("the snapshot is there");

    assert_eq!(
        found
            .iter()
            .map(|entry| entry.metadata.context.clone())
            .collect::<Vec<_>>(),
        vec![
            context.clone(),
            context.clone(),
            AggregateContext::default()
        ]
    );
    // and: so does the latest snapshot
    assert_eq!(latest.metadata.context, context);
}

#[macro_export]
macro_rules! conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::testing::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::testing::suite::$case(&bench.committing()).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::conformance_case!($workbench, an_empty_stream_is_at_version_zero);
        $crate::conformance_case!($workbench, appended_events_read_back_in_order);
        $crate::conformance_case!(
            $workbench,
            an_event_s_context_is_read_back_as_it_was_written
        );
        $crate::conformance_case!($workbench, appending_at_a_stale_version_is_refused);
        $crate::conformance_case!($workbench, a_refused_append_leaves_the_stream_untouched);
        $crate::conformance_case!($workbench, one_append_may_carry_several_events);
        $crate::conformance_case!($workbench, a_pruned_stream_cannot_be_started_over);
        $crate::conformance_case!(
            $workbench,
            a_forgotten_stream_is_gone_whole_and_the_others_stay
        );
        $crate::conformance_case!($workbench, streams_of_different_aggregates_do_not_mix);
        $crate::conformance_case!($workbench, reading_from_a_version_skips_what_came_before);
        $crate::conformance_case!($workbench, reading_through_a_version_stops_there);
        $crate::conformance_case!($workbench, a_stream_without_snapshots_has_none);
        $crate::conformance_case!($workbench, the_latest_snapshot_is_the_newest_one);
        $crate::conformance_case!($workbench, a_snapshot_can_be_found_as_of_an_older_version);
        $crate::conformance_case!($workbench, pruning_discards_events_a_snapshot_replaced);
        $crate::conformance_case!(
            $workbench,
            appending_after_pruning_continues_the_version_count
        );
        $crate::bench_case!(
            $workbench,
            a_read_within_a_transaction_sees_what_it_appended
        );
        $crate::bench_case!(
            $workbench,
            an_append_leaves_a_message_waiting_for_every_publishable_event
        );
        $crate::bench_case!(
            $workbench,
            an_event_that_publishes_nothing_leaves_nothing_waiting
        );
        $crate::bench_case!($workbench, a_refused_append_leaves_no_message_waiting);
        $crate::bench_case!(
            $workbench,
            a_dropped_append_leaves_neither_its_events_nor_their_messages
        );
        $crate::bench_case!(
            $workbench,
            a_dropped_append_onto_a_stream_leaves_the_stream_as_it_was
        );
        $crate::bench_case!(
            $workbench,
            a_dropped_prune_keeps_what_it_would_have_discarded
        );
        $crate::bench_case!($workbench, a_dropped_forget_keeps_the_stream);
    };
}

#[macro_export]
macro_rules! bench_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test(flavor = "multi_thread")]
        async fn $case() {
            use $crate::testing::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::testing::suite::$case(&bench).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! table_tests {
    ($workbench:ty) => {
        $crate::bench_case!($workbench, two_writers_at_the_same_version_cannot_both_win);
        $crate::bench_case!(
            $workbench,
            a_batch_that_collides_halfway_writes_none_of_itself
        );
        $crate::bench_case!($workbench, a_backend_that_cannot_be_reached_says_so);
        $crate::bench_case!(
            $workbench,
            an_event_written_in_a_shape_nothing_can_read_is_a_backend_failure
        );
        $crate::bench_case!($workbench, what_the_columns_say_matches_what_the_body_says);
        $crate::bench_case!(
            $workbench,
            a_waiting_message_keeps_the_version_of_the_event_it_tells
        );
        $crate::bench_case!($workbench, a_codec_is_all_a_second_kind_of_event_needs);
        $crate::bench_case!($workbench, laying_the_schema_down_twice_changes_nothing);
        $crate::bench_case!(
            $workbench,
            a_version_below_zero_is_a_backend_failure_not_a_huge_version
        );
    };
}
