use crate::aggregate::AggregateId;
use crate::event::Recorded;
use crate::store::{EventStore, StoreError};
use crate::testing::sample::{SAMPLE, SampleEvent, recorded, stamped};
use crate::version::Version;

fn a_creation() -> SampleEvent {
    SampleEvent::Created {
        title: "The Loom".to_owned(),
        description: "A silent machine.".to_owned(),
    }
}

fn a_snapshot() -> SampleEvent {
    SampleEvent::Snapshotted {
        title: "The Loom".to_owned(),
        description: "A silent machine.".to_owned(),
        deleted: false,
    }
}

fn at(aggregate: &AggregateId, version: u64, event: SampleEvent) -> Recorded<SampleEvent> {
    let mut metadata = stamped(aggregate, version);
    metadata.is_snapshot = matches!(event, SampleEvent::Snapshotted { .. });

    Recorded { event, metadata }
}

async fn given_a_stream(
    store: &impl EventStore<SampleEvent>,
    aggregate: &AggregateId,
    events: Vec<SampleEvent>,
) {
    let stream = recorded(aggregate, events);

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

pub async fn an_empty_stream_is_at_version_zero(store: &impl EventStore<SampleEvent>) {
    let aggregate = AggregateId::from("sample_empty");

    let found = store
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await
        .expect("reading an unknown stream should not fail");

    assert!(
        found.is_empty(),
        "a stream nobody has written to must read as empty, not as missing"
    );
}

pub async fn appended_events_read_back_in_order(store: &impl EventStore<SampleEvent>) {
    let aggregate = AggregateId::from("sample_ordered");
    given_a_stream(
        store,
        &aggregate,
        vec![
            a_creation(),
            SampleEvent::TitleUpdated("The Silent Loom".to_owned()),
            SampleEvent::DescriptionUpdated("It remembers.".to_owned()),
        ],
    )
    .await;

    let found = store
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");

    assert_eq!(found.len(), 3);
    assert_eq!(found[0].event, a_creation());
    assert_eq!(
        found[2].event,
        SampleEvent::DescriptionUpdated("It remembers.".to_owned())
    );
}

pub async fn appending_at_a_stale_version_is_refused(store: &impl EventStore<SampleEvent>) {
    let aggregate = AggregateId::from("sample_stale");
    given_a_stream(store, &aggregate, vec![a_creation()]).await;

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

    assert_eq!(
        refused,
        Err(StoreError::Outdated {
            aggregate: aggregate.clone(),
            kind: SAMPLE,
            expected: Version::ZERO,
        })
    );
}

pub async fn a_refused_append_leaves_the_stream_untouched(store: &impl EventStore<SampleEvent>) {
    let aggregate = AggregateId::from("sample_untouched");
    given_a_stream(store, &aggregate, vec![a_creation()]).await;

    let _ = store
        .append(
            &aggregate,
            SAMPLE,
            Version::of(7),
            &[at(&aggregate, 8, SampleEvent::Deleted)],
        )
        .await;

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

pub async fn one_append_may_carry_several_events(store: &impl EventStore<SampleEvent>) {
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

    let found = store
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");

    assert_eq!(found.len(), 2, "a batch must be stored whole");
}

pub async fn streams_of_different_aggregates_do_not_mix(store: &impl EventStore<SampleEvent>) {
    let one = AggregateId::from("sample_one");
    let other = AggregateId::from("sample_other");
    given_a_stream(store, &one, vec![a_creation()]).await;
    given_a_stream(
        store,
        &other,
        vec![a_creation(), SampleEvent::TitleUpdated("Apart".to_owned())],
    )
    .await;

    let first = store
        .read_from(&one, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");
    let second = store
        .read_from(&other, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");

    assert_eq!(first.len(), 1);
    assert_eq!(second.len(), 2);
}

pub async fn reading_from_a_version_skips_what_came_before(store: &impl EventStore<SampleEvent>) {
    let aggregate = AggregateId::from("sample_from");
    given_a_stream(
        store,
        &aggregate,
        vec![
            a_creation(),
            SampleEvent::TitleUpdated("Second".to_owned()),
            SampleEvent::TitleUpdated("Third".to_owned()),
        ],
    )
    .await;

    let found = store
        .read_from(&aggregate, SAMPLE, Version::of(2))
        .await
        .expect("reading should succeed");

    assert_eq!(
        found.len(),
        2,
        "reading from a version includes that version"
    );
    assert_eq!(found[0].metadata.version, Version::of(2));
}

pub async fn reading_through_a_version_stops_there(store: &impl EventStore<SampleEvent>) {
    let aggregate = AggregateId::from("sample_through");
    given_a_stream(
        store,
        &aggregate,
        vec![
            a_creation(),
            SampleEvent::TitleUpdated("Second".to_owned()),
            SampleEvent::TitleUpdated("Third".to_owned()),
        ],
    )
    .await;

    let found = store
        .read_through(&aggregate, SAMPLE, Version::ZERO, Version::of(2))
        .await
        .expect("reading should succeed");

    assert_eq!(found.len(), 2, "reading through a version includes it");
    assert_eq!(found[1].metadata.version, Version::of(2));
}

pub async fn a_stream_without_snapshots_has_none(store: &impl EventStore<SampleEvent>) {
    let aggregate = AggregateId::from("sample_nosnap");
    given_a_stream(store, &aggregate, vec![a_creation()]).await;

    let found = store
        .latest_snapshot(&aggregate, SAMPLE)
        .await
        .expect("looking for a snapshot should not fail");

    assert!(found.is_none());
}

pub async fn the_latest_snapshot_is_the_newest_one(store: &impl EventStore<SampleEvent>) {
    let aggregate = AggregateId::from("sample_latest");
    given_a_stream(
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

    let found = store
        .latest_snapshot(&aggregate, SAMPLE)
        .await
        .expect("looking for a snapshot should not fail")
        .expect("this stream has snapshots");

    assert_eq!(found.metadata.version, Version::of(4));
}

pub async fn a_snapshot_can_be_found_as_of_an_older_version(store: &impl EventStore<SampleEvent>) {
    let aggregate = AggregateId::from("sample_asof");
    given_a_stream(
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

    let found = store
        .snapshot_at_or_before(&aggregate, SAMPLE, Version::of(3))
        .await
        .expect("looking for a snapshot should not fail")
        .expect("there is a snapshot at or before version 3");

    assert_eq!(
        found.metadata.version,
        Version::of(2),
        "time travel must not see a snapshot from the future"
    );
}

pub async fn pruning_discards_events_a_snapshot_replaced(store: &impl EventStore<SampleEvent>) {
    let aggregate = AggregateId::from("sample_pruned");
    given_a_stream(
        store,
        &aggregate,
        vec![
            a_creation(),
            SampleEvent::TitleUpdated("Second".to_owned()),
            a_snapshot(),
        ],
    )
    .await;

    store
        .prune_through(&aggregate, SAMPLE, Version::of(2))
        .await
        .expect("pruning should succeed");

    let found = store
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");

    assert_eq!(found.len(), 1);
    assert_eq!(found[0].metadata.version, Version::of(3));
}

pub async fn appending_after_pruning_continues_the_version_count(
    store: &impl EventStore<SampleEvent>,
) {
    let aggregate = AggregateId::from("sample_after_prune");
    given_a_stream(
        store,
        &aggregate,
        vec![a_creation(), SampleEvent::TitleUpdated("Second".to_owned())],
    )
    .await;

    store
        .prune_through(&aggregate, SAMPLE, Version::of(1))
        .await
        .expect("pruning should succeed");

    store
        .append(
            &aggregate,
            SAMPLE,
            Version::of(2),
            &[at(&aggregate, 3, SampleEvent::Deleted)],
        )
        .await
        .expect("a pruned stream must remember how far it got");

    let found = store
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");

    assert_eq!(
        found.last().expect("not empty").metadata.version,
        Version::of(3)
    );
}

#[macro_export]
macro_rules! conformance_case {
    ($make_store:expr, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            $crate::testing::suite::$case(&$make_store).await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($make_store:expr) => {
        $crate::conformance_case!($make_store, an_empty_stream_is_at_version_zero);
        $crate::conformance_case!($make_store, appended_events_read_back_in_order);
        $crate::conformance_case!($make_store, appending_at_a_stale_version_is_refused);
        $crate::conformance_case!($make_store, a_refused_append_leaves_the_stream_untouched);
        $crate::conformance_case!($make_store, one_append_may_carry_several_events);
        $crate::conformance_case!($make_store, streams_of_different_aggregates_do_not_mix);
        $crate::conformance_case!($make_store, reading_from_a_version_skips_what_came_before);
        $crate::conformance_case!($make_store, reading_through_a_version_stops_there);
        $crate::conformance_case!($make_store, a_stream_without_snapshots_has_none);
        $crate::conformance_case!($make_store, the_latest_snapshot_is_the_newest_one);
        $crate::conformance_case!($make_store, a_snapshot_can_be_found_as_of_an_older_version);
        $crate::conformance_case!($make_store, pruning_discards_events_a_snapshot_replaced);
        $crate::conformance_case!(
            $make_store,
            appending_after_pruning_continues_the_version_count
        );
    };
}
