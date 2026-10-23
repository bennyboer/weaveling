use crate::aggregate::AggregateId;
use crate::codec::Codec;
use crate::event::{Event, Recorded};
use crate::store::{EventStore, StoreError};
use crate::testing::sample;
use crate::testing::sample::{SAMPLE, SampleEvent, SampleKind};
use crate::testing::{Tables, Workbench};
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
    }
}

fn at(aggregate: &AggregateId, version: u64, event: SampleEvent) -> Recorded<SampleEvent> {
    let mut metadata = sample::stamped(aggregate, version);
    metadata.is_snapshot = matches!(event, SampleEvent::Snapshotted { .. });

    Recorded { event, metadata }
}

async fn given_a_stream(
    store: &impl EventStore<SampleEvent>,
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

pub async fn a_pruned_stream_cannot_be_started_over(store: &impl EventStore<SampleEvent>) {
    let aggregate = AggregateId::from("sample_looks_empty");
    given_a_stream(
        store,
        &aggregate,
        vec![a_creation(), SampleEvent::Deleted, a_snapshot()],
    )
    .await;

    store
        .prune_through(&aggregate, SAMPLE, Version::of(2))
        .await
        .expect("pruning should succeed");

    let refused = store
        .append(
            &aggregate,
            SAMPLE,
            Version::ZERO,
            &[at(&aggregate, 1, a_creation())],
        )
        .await;

    assert!(
        matches!(refused, Err(StoreError::Outdated { .. })),
        "a stream collapsed to a snapshot has a history, however little of it is left: {refused:?}"
    );

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

fn retitled(title: &str) -> SampleEvent {
    SampleEvent::TitleUpdated(title.to_owned())
}

async fn titles_in(store: &impl EventStore<SampleEvent>, aggregate: &AggregateId) -> Vec<String> {
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

pub async fn an_append_leaves_a_message_waiting_for_every_publishable_event(
    bench: &impl Workbench,
) {
    let aggregate = AggregateId::from("sample_announced");

    given_a_stream(
        bench.store(),
        &aggregate,
        vec![a_creation(), retitled("Second")],
    )
    .await;

    assert_eq!(
        bench.waiting().await,
        ["sample.created", "sample.title_updated"],
        "an append enqueues, it does not publish"
    );
}

pub async fn an_event_that_publishes_nothing_leaves_nothing_waiting(bench: &impl Workbench) {
    let aggregate = AggregateId::from("sample_quiet");

    given_a_stream(
        bench.store(),
        &aggregate,
        vec![
            a_creation(),
            SampleEvent::Corrected("Quietly".to_owned()),
            a_snapshot(),
        ],
    )
    .await;

    assert_eq!(
        bench.waiting().await,
        ["sample.created"],
        "a correction and a snapshot are both unpublishable, so only the creation waits"
    );
}

pub async fn a_refused_append_leaves_no_message_waiting(bench: &impl Workbench) {
    let aggregate = AggregateId::from("sample_refused");

    let refused = bench
        .store()
        .append(
            &aggregate,
            SAMPLE,
            Version::of(7),
            &[at(&aggregate, 8, a_creation())],
        )
        .await;

    assert!(refused.is_err());
    assert!(
        bench.waiting().await.is_empty(),
        "the event and its message are written together or not at all"
    );
}

pub async fn two_writers_at_the_same_version_cannot_both_win(bench: &impl Tables) {
    let aggregate = AggregateId::from("sample_contested");
    given_a_stream(bench.store(), &aggregate, vec![a_creation()]).await;
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

    let (mine, yours) = tokio::join!(
        bench
            .store()
            .append(&aggregate, SAMPLE, Version::of(1), &mine),
        bench
            .store()
            .append(&aggregate, SAMPLE, Version::of(1), &yours),
    );

    assert_ne!(
        mine.is_ok(),
        yours.is_ok(),
        "exactly one writer should win, got {mine:?} and {yours:?}"
    );
    let refused = mine.err().or(yours.err()).expect("one of them was refused");
    assert!(
        matches!(refused, StoreError::Outdated { .. }),
        "a lost race is a stale version, not a backend failure: {refused:?}"
    );
    assert_eq!(
        titles_in(bench.store(), &aggregate).await.len(),
        1,
        "the loser's events must not be mixed into the winner's"
    );
}

pub async fn a_batch_that_collides_halfway_writes_none_of_itself(bench: &impl Tables) {
    let aggregate = AggregateId::from("sample_half");
    given_a_stream(bench.store(), &aggregate, vec![a_creation()]).await;

    let refused = bench
        .store()
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

    assert!(
        matches!(refused, Err(StoreError::Outdated { .. })),
        "a batch colliding with what is already written is a stale append: {refused:?}"
    );
    assert!(
        titles_in(bench.store(), &aggregate).await.is_empty(),
        "an append is all of its events or none of them"
    );
    assert_eq!(
        bench.waiting().await,
        ["sample.created"],
        "only the stream's start is announced: the outbox row shares its append's transaction"
    );
}

pub async fn a_backend_that_cannot_be_reached_says_so(bench: &impl Tables) {
    let aggregate = AggregateId::from("sample_unreachable");

    bench.close().await;

    let reading = bench
        .store()
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await;
    let appending = bench
        .store()
        .append(
            &aggregate,
            SAMPLE,
            Version::ZERO,
            &[at(&aggregate, 1, a_creation())],
        )
        .await;
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
    let aggregate = AggregateId::from("sample_unreadable");
    given_a_stream(bench.store(), &aggregate, vec![a_creation()]).await;

    bench.write_unreadable_event(&aggregate, 2).await;

    let found = bench
        .store()
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await;
    assert!(
        matches!(found, Err(StoreError::Backend { .. })),
        "an unreadable row should be reported, not skipped or panicked on: {found:?}"
    );
}

pub async fn what_the_columns_say_matches_what_the_body_says(bench: &impl Tables) {
    let aggregate = AggregateId::from("sample_columns");

    given_a_stream(bench.store(), &aggregate, vec![a_creation()]).await;

    let (name, shape, agent) = bench.columns(&aggregate, 1).await;
    assert_eq!(name, a_creation().name().as_str());
    assert_eq!(shape as u64, a_creation().version().count());
    assert_eq!(agent, "system");
}

pub async fn a_codec_is_all_a_second_kind_of_event_needs(bench: &impl Tables) {
    let store = bench.reading_with(Codec {
        body: |_| serde_json::json!("Deleted"),
        event: |_| Some(SampleEvent::Deleted),
    });
    let aggregate = AggregateId::from("sample_recoded");

    given_a_stream(&store, &aggregate, vec![a_creation()]).await;

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
    bench.lay_the_schema_down_again().await;

    given_a_stream(
        bench.store(),
        &AggregateId::from("sample_again"),
        vec![a_creation()],
    )
    .await;
}

pub async fn a_version_below_zero_is_a_backend_failure_not_a_huge_version(bench: &impl Tables) {
    let aggregate = AggregateId::from("sample_negative");
    given_a_stream(bench.store(), &aggregate, vec![a_creation()]).await;

    bench.push_versions_below_zero(&aggregate).await;

    let found = bench.store().latest_snapshot(&aggregate, SAMPLE).await;
    assert!(
        matches!(found, Err(StoreError::Backend { .. })),
        "-3 read as a u64 is eighteen quintillion, a version nothing could ever have written: {found:?}"
    );
}

#[macro_export]
macro_rules! conformance_case {
    ($workbench:ty, $case:ident) => {
        #[tokio::test]
        async fn $case() {
            use $crate::testing::Workbench;

            let bench = <$workbench>::setup().await;
            $crate::testing::suite::$case(bench.store()).await;
            bench.cleanup().await;
        }
    };
}

#[macro_export]
macro_rules! conformance_tests {
    ($workbench:ty) => {
        $crate::conformance_case!($workbench, an_empty_stream_is_at_version_zero);
        $crate::conformance_case!($workbench, appended_events_read_back_in_order);
        $crate::conformance_case!($workbench, appending_at_a_stale_version_is_refused);
        $crate::conformance_case!($workbench, a_refused_append_leaves_the_stream_untouched);
        $crate::conformance_case!($workbench, one_append_may_carry_several_events);
        $crate::conformance_case!($workbench, a_pruned_stream_cannot_be_started_over);
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
            an_append_leaves_a_message_waiting_for_every_publishable_event
        );
        $crate::bench_case!(
            $workbench,
            an_event_that_publishes_nothing_leaves_nothing_waiting
        );
        $crate::bench_case!($workbench, a_refused_append_leaves_no_message_waiting);
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
        $crate::bench_case!($workbench, a_codec_is_all_a_second_kind_of_event_needs);
        $crate::bench_case!($workbench, laying_the_schema_down_twice_changes_nothing);
        $crate::bench_case!(
            $workbench,
            a_version_below_zero_is_a_backend_failure_not_a_huge_version
        );
    };
}
