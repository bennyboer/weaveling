use crate::context::AggregateContext;
use std::sync::Arc;

use async_trait::async_trait;
use sqlx::SqlitePool;
use test_harness::SqliteFixture;

use time::{Duration, OffsetDateTime};

use crate::agent::Agent;
use crate::aggregate::AggregateId;
use crate::codec::Codec;
use crate::event::{Event, Recorded};
use crate::metadata::EventMetadata;
use crate::sqlite::SqliteEventStore;
use crate::store::{EventStore, StoreError};
use crate::testing::sample;
use crate::testing::sample::{SAMPLE, SampleEvent, SampleKind};
use crate::testing::stored;
use crate::testing::{Tables, Workbench};
use crate::version::Version;

struct OnSqlite {
    fixture: SqliteFixture,
    pool: SqlitePool,
    store: SqliteEventStore<SampleEvent>,
}

async fn ready_for(feature: &str, fixture: &SqliteFixture) -> SqlitePool {
    let pool = fixture.create_database(feature).await;
    crate::sqlite::migrations()
        .run(&pool)
        .await
        .expect("the event store's schema should lay down in an empty file");
    outbox::sqlite::migrations()
        .run(&pool)
        .await
        .expect("the outbox an append writes into should lay down beside it");

    pool
}

#[async_trait]
impl Workbench for OnSqlite {
    type Store = SqliteEventStore<SampleEvent>;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool = ready_for("sample", &fixture).await;
        let store = SqliteEventStore::new(pool.clone(), stored::codec(), sample::message_for);

        Self {
            fixture,
            pool,
            store,
        }
    }

    fn store(&self) -> &Self::Store {
        &self.store
    }

    async fn waiting(&self) -> Vec<String> {
        sqlx::query_scalar(
            "SELECT routing_key FROM outbox WHERE published_at IS NULL ORDER BY entry",
        )
        .fetch_all(&self.pool)
        .await
        .expect("reading the outbox should succeed")
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

#[async_trait]
impl Tables for OnSqlite {
    fn reading_with(&self, codec: Codec<SampleEvent>) -> Self::Store {
        SqliteEventStore::new(self.pool.clone(), codec, sample::message_for)
    }

    async fn close(&self) {
        self.pool.close().await;
    }

    async fn lay_the_schema_down_again(&self) {
        crate::sqlite::migrations()
            .run(&self.pool)
            .await
            .expect("a schema already laid down should be left alone");
    }

    async fn write_unreadable_event(&self, aggregate: &AggregateId, version: u64) {
        sqlx::query(
            "INSERT INTO events
                (aggregate, kind, version, name, body, body_version, agent, occurred_at, is_snapshot)
             VALUES (?1, ?2, ?3, 'CREATED', ?4, 0, 'system', '1970-01-01T00:00:00.000000000Z', 0)",
        )
        .bind(aggregate.as_str())
        .bind(SAMPLE.as_str())
        .bind(version as i64)
        .bind(stored::nonsense().to_string())
        .execute(&self.pool)
        .await
        .expect("writing a row by hand should succeed");
    }

    async fn push_versions_below_zero(&self, aggregate: &AggregateId) {
        sqlx::query("UPDATE events SET version = -3, is_snapshot = 1 WHERE aggregate = ?1")
            .bind(aggregate.as_str())
            .execute(&self.pool)
            .await
            .expect("editing the file by hand should succeed");
    }

    async fn columns(&self, aggregate: &AggregateId, version: u64) -> (String, i64, String) {
        sqlx::query_as(
            "SELECT name, body_version, agent FROM events WHERE aggregate = ?1 AND version = ?2",
        )
        .bind(aggregate.as_str())
        .bind(version as i64)
        .fetch_one(&self.pool)
        .await
        .expect("the row just written should be there")
    }
}

crate::conformance_tests!(OnSqlite);
crate::table_tests!(OnSqlite);

fn a_creation() -> SampleEvent {
    SampleEvent::Created {
        title: "The Loom".to_owned(),
        description: "A silent machine.".to_owned(),
        kind: SampleKind::Ordinary,
    }
}

fn retitled(title: &str) -> SampleEvent {
    SampleEvent::TitleUpdated(title.to_owned())
}

fn at(aggregate: &AggregateId, version: u64, event: SampleEvent) -> Recorded<SampleEvent> {
    Recorded {
        metadata: EventMetadata {
            aggregate: aggregate.clone(),
            kind: SAMPLE,
            version: Version::of(version),
            agent: Agent::System,
            occurred_at: OffsetDateTime::UNIX_EPOCH
                + Duration::seconds(1_759_000_000)
                + Duration::nanoseconds(123_456_789),
            is_snapshot: event.is_snapshot(),
            context: AggregateContext::default(),
        },
        event,
    }
}

async fn a_started_stream(store: &SqliteEventStore<SampleEvent>, aggregate: &AggregateId) {
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

async fn titles_in(store: &SqliteEventStore<SampleEvent>, aggregate: &AggregateId) -> Vec<String> {
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

#[tokio::test]
async fn an_event_keeps_the_instant_it_happened_to_the_nanosecond() {
    let bench = OnSqlite::setup().await;
    let aggregate = AggregateId::from("sample_instant");
    let written = at(&aggregate, 1, a_creation());

    bench
        .store()
        .append(
            &aggregate,
            SAMPLE,
            Version::ZERO,
            std::slice::from_ref(&written),
        )
        .await
        .expect("appending should succeed");
    let found = bench
        .store()
        .read_from(&aggregate, SAMPLE, Version::ZERO)
        .await
        .expect("reading should succeed");

    assert_eq!(found[0].metadata, written.metadata);

    bench.cleanup().await;
}

#[tokio::test]
async fn the_event_store_leaves_the_usual_ledger_free_for_whoever_owns_the_file() {
    let fixture = SqliteFixture::setup();
    let pool = ready_for("sharing", &fixture).await;

    let found: Vec<String> =
        sqlx::query_scalar("SELECT name FROM sqlite_master WHERE type = 'table' ORDER BY name")
            .fetch_all(&pool)
            .await
            .expect("reading the catalog should succeed");

    assert!(found.contains(&"events".to_owned()) && found.contains(&"outbox".to_owned()));
    assert!(
        found.contains(&"_sqlx_migrations_events".to_owned()),
        "{found:?}"
    );
    assert!(
        !found.contains(&"_sqlx_migrations".to_owned()),
        "the default ledger belongs to the feature whose file this is, found {found:?}"
    );

    fixture.cleanup().await;
}

#[tokio::test]
async fn the_schema_carries_the_indexes_the_queries_rely_on() {
    let fixture = SqliteFixture::setup();
    let pool = ready_for("indexed", &fixture).await;

    let found: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'index' AND sql IS NOT NULL ORDER BY name",
    )
    .fetch_all(&pool)
    .await
    .expect("reading the catalog should succeed");

    assert_eq!(
        found,
        vec!["events_snapshots", "outbox_published", "outbox_waiting"],
        "sqlx bakes the migrations into the binary at compile time and cannot tell cargo the SQL \
         is an input, so an edited migration can go missing from a build entirely"
    );

    fixture.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_writer_kept_waiting_by_another_is_told_it_is_stale_not_that_the_file_was_busy() {
    let bench = OnSqlite::setup().await;
    let aggregate = AggregateId::from("sample_waiting");
    a_started_stream(bench.store(), &aggregate).await;
    let mut other = bench
        .pool
        .begin_with("BEGIN IMMEDIATE")
        .await
        .expect("the other writer takes the lock");
    sqlx::query(
        "INSERT INTO events
            (aggregate, kind, version, name, body, body_version, agent, occurred_at, is_snapshot)
         VALUES (?1, ?2, 2, 'TITLE_UPDATED', ?3, 0, 'system', '1970-01-01T00:00:00.000000000Z', 0)",
    )
    .bind(aggregate.as_str())
    .bind(SAMPLE.as_str())
    .bind((stored::codec().body)(&retitled("Theirs")).to_string())
    .execute(&mut *other)
    .await
    .expect("the other writer writes");

    let store = Arc::new(SqliteEventStore::new(
        bench.pool.clone(),
        stored::codec(),
        sample::message_for,
    ));
    let appending = tokio::spawn({
        let aggregate = aggregate.clone();
        let batch = vec![at(&aggregate, 2, retitled("Mine"))];

        async move {
            store
                .append(&aggregate, SAMPLE, Version::of(1), &batch)
                .await
        }
    });
    tokio::time::sleep(std::time::Duration::from_millis(200)).await;
    other.commit().await.expect("the other writer commits");

    let mine = appending.await.expect("the append finishes");

    assert!(
        matches!(mine, Err(StoreError::Outdated { .. })),
        "a deferred transaction that read the head before the other commit could not upgrade to \
         a writer, and SQLite says busy rather than waiting: {mine:?}"
    );
    assert_eq!(titles_in(bench.store(), &aggregate).await, vec!["Theirs"]);

    bench.cleanup().await;
}
