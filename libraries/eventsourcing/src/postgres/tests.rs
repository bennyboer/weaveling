use std::sync::Arc;

use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;
use time::OffsetDateTime;

use crate::agent::Agent;
use crate::aggregate::AggregateId;
use crate::event::{Event, Recorded};
use crate::metadata::EventMetadata;
use crate::postgres::sample::{codec, message_for, nonsense};
use crate::postgres::{Codec, PostgresEventStore};
use crate::store::{EventStore, StoreError};
use crate::testing::Workbench;
use crate::testing::sample::{SAMPLE, SampleEvent};
use crate::version::Version;

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    store: PostgresEventStore<SampleEvent>,
}

async fn ready_for(feature: &str, fixture: &PostgresFixture) -> PgPool {
    let pool = fixture.create_schema(feature).await;
    crate::postgres::migrations()
        .run(&pool)
        .await
        .expect("the event store's schema should lay down in an empty namespace");

    pool
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresEventStore<SampleEvent>;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool = ready_for("sample", &fixture).await;
        let store = PostgresEventStore::new(pool.clone(), codec(), message_for);

        Self {
            fixture,
            pool,
            store,
        }
    }

    fn store(&self) -> &Self::Store {
        &self.store
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

crate::conformance_tests!(OnPostgres);

fn at(aggregate: &AggregateId, version: u64, event: SampleEvent) -> Recorded<SampleEvent> {
    Recorded {
        metadata: EventMetadata {
            aggregate: aggregate.clone(),
            kind: SAMPLE,
            version: Version::of(version),
            agent: Agent::System,
            occurred_at: OffsetDateTime::UNIX_EPOCH,
            is_snapshot: event.is_snapshot(),
        },
        event,
    }
}

fn a_creation() -> SampleEvent {
    SampleEvent::Created {
        title: "The Loom".to_owned(),
        description: "A silent machine.".to_owned(),
        kind: crate::testing::sample::SampleKind::Ordinary,
    }
}

async fn a_started_stream(store: &PostgresEventStore<SampleEvent>, aggregate: &AggregateId) {
    store
        .append(
            aggregate,
            SAMPLE,
            Version::ZERO,
            &[at(aggregate, 1, a_creation())],
        )
        .await
        .expect("starting a stream should succeed");
}

async fn titles_in(
    store: &PostgresEventStore<SampleEvent>,
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

#[tokio::test(flavor = "multi_thread")]
async fn two_writers_at_the_same_version_cannot_both_win() {
    let bench = OnPostgres::setup().await;
    let aggregate = AggregateId::from("sample_contested");
    a_started_stream(bench.store(), &aggregate).await;

    let store = Arc::new(PostgresEventStore::new(
        bench.pool.clone(),
        codec(),
        message_for,
    ));
    let batch = |named: &str| {
        vec![
            at(&aggregate, 2, SampleEvent::TitleUpdated(named.to_owned())),
            at(
                &aggregate,
                3,
                SampleEvent::DescriptionUpdated(named.to_owned()),
            ),
        ]
    };

    let (mine, yours) = tokio::join!(
        {
            let store = store.clone();
            let aggregate = aggregate.clone();
            let batch = batch("Mine");

            async move {
                store
                    .append(&aggregate, SAMPLE, Version::of(1), &batch)
                    .await
            }
        },
        {
            let store = store.clone();
            let aggregate = aggregate.clone();
            let batch = batch("Yours");

            async move {
                store
                    .append(&aggregate, SAMPLE, Version::of(1), &batch)
                    .await
            }
        }
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

    bench.cleanup().await;
}

#[tokio::test]
async fn a_batch_that_collides_halfway_writes_none_of_itself() {
    let bench = OnPostgres::setup().await;
    let aggregate = AggregateId::from("sample_half");
    a_started_stream(bench.store(), &aggregate).await;

    let refused = bench
        .store()
        .append(
            &aggregate,
            SAMPLE,
            Version::of(1),
            &[
                at(
                    &aggregate,
                    2,
                    SampleEvent::TitleUpdated("Second".to_owned()),
                ),
                at(
                    &aggregate,
                    1,
                    SampleEvent::TitleUpdated("First again".to_owned()),
                ),
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

    bench.cleanup().await;
}

#[tokio::test]
async fn a_backend_that_cannot_be_reached_says_so() {
    let bench = OnPostgres::setup().await;
    let aggregate = AggregateId::from("sample_unreachable");

    bench.pool.close().await;

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

    bench.cleanup().await;
}

#[tokio::test]
async fn an_event_written_in_a_shape_nothing_can_read_is_a_backend_failure() {
    let bench = OnPostgres::setup().await;
    let aggregate = AggregateId::from("sample_unreadable");
    a_started_stream(bench.store(), &aggregate).await;

    sqlx::query(
        "INSERT INTO events
            (aggregate, kind, version, name, body, body_version, agent, occurred_at, is_snapshot)
         VALUES ($1, $2, 2, 'CREATED', $3, 0, 'system', NOW(), false)",
    )
    .bind(aggregate.as_str())
    .bind(SAMPLE.as_str())
    .bind(nonsense())
    .execute(&bench.pool)
    .await
    .expect("writing a row by hand should succeed");

    let found = bench
        .store()
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await;

    assert!(
        matches!(found, Err(StoreError::Backend { .. })),
        "an unreadable row should be reported, not skipped or panicked on: {found:?}"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn what_the_columns_say_matches_what_the_body_says() {
    let bench = OnPostgres::setup().await;
    let aggregate = AggregateId::from("sample_columns");
    a_started_stream(bench.store(), &aggregate).await;

    let (name, shape, agent): (String, i64, String) = sqlx::query_as(
        "SELECT name, body_version, agent FROM events WHERE aggregate = $1 AND version = 1",
    )
    .bind(aggregate.as_str())
    .fetch_one(&bench.pool)
    .await
    .expect("the row we just wrote should be there");

    assert_eq!(name, a_creation().name().as_str());
    assert_eq!(shape as u64, a_creation().version().count());
    assert_eq!(agent, "system");

    bench.cleanup().await;
}

#[tokio::test]
async fn a_codec_is_all_a_second_kind_of_event_needs() {
    let fixture = PostgresFixture::setup().await;
    let store: PostgresEventStore<SampleEvent> = PostgresEventStore::new(
        ready_for("recoded", &fixture).await,
        Codec {
            body: |_| serde_json::json!("Deleted"),
            event: |_| Some(SampleEvent::Deleted),
        },
        message_for,
    );
    let aggregate = AggregateId::from("sample_recoded");

    store
        .append(
            &aggregate,
            SAMPLE,
            Version::ZERO,
            &[at(&aggregate, 1, a_creation())],
        )
        .await
        .expect("appending should succeed");

    let found = store
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");

    assert_eq!(
        found[0].event,
        SampleEvent::Deleted,
        "the SQL is written once and the shape on disk belongs to whoever supplied the codec"
    );

    fixture.cleanup().await;
}

async fn tables_in(pool: &PgPool, schema: &str) -> Vec<String> {
    sqlx::query_scalar::<_, String>("SELECT tablename::text FROM pg_tables WHERE schemaname = $1")
        .bind(schema)
        .fetch_all(pool)
        .await
        .expect("reading the catalog should succeed")
}

#[tokio::test]
async fn the_event_store_leaves_the_usual_ledger_free_for_whoever_owns_the_database() {
    let fixture = PostgresFixture::setup().await;
    let pool = ready_for("sharing", &fixture).await;

    let found = tables_in(&pool, &fixture.schema_of("sharing")).await;

    assert!(found.contains(&"events".to_owned()) && found.contains(&"outbox".to_owned()));
    assert!(
        found.contains(&"_sqlx_migrations_events".to_owned()),
        "the event store keeps its own ledger, found {found:?}"
    );
    assert!(
        !found.contains(&"_sqlx_migrations".to_owned()),
        "the default ledger belongs to the feature whose database this is, found {found:?}"
    );

    fixture.cleanup().await;
}

#[tokio::test]
async fn laying_the_schema_down_twice_changes_nothing() {
    let fixture = PostgresFixture::setup().await;
    let pool = ready_for("again", &fixture).await;

    crate::postgres::migrations()
        .run(&pool)
        .await
        .expect("a schema already laid down should be left alone");

    let aggregate = AggregateId::from("sample_again");
    a_started_stream(
        &PostgresEventStore::new(pool.clone(), codec(), message_for),
        &aggregate,
    )
    .await;

    fixture.cleanup().await;
}

#[tokio::test]
async fn the_schema_carries_the_indexes_the_queries_rely_on() {
    let fixture = PostgresFixture::setup().await;
    let pool = ready_for("indexed", &fixture).await;

    let found: Vec<String> = sqlx::query_scalar(
        "SELECT indexname::text FROM pg_indexes WHERE schemaname = $1 ORDER BY indexname",
    )
    .bind(fixture.schema_of("indexed"))
    .fetch_all(&pool)
    .await
    .expect("reading the catalog should succeed");

    assert_eq!(
        found,
        vec![
            "_sqlx_migrations_events_pkey",
            "events_pkey",
            "events_snapshots",
            "outbox_pkey",
            "outbox_published",
            "outbox_waiting",
        ],
        "sqlx bakes the migrations into the binary at compile time and cannot tell cargo the SQL \
         is an input, so an edited migration can go missing from a build entirely"
    );

    fixture.cleanup().await;
}
