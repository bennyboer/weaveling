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
use crate::testing::Workbench;
use crate::testing::sample::{SAMPLE, SampleEvent};
use crate::testing::stored::{codec, message_for, nonsense};
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
        let store = SqliteEventStore::new(pool.clone(), codec(), message_for);

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

crate::conformance_tests!(OnSqlite);

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

fn retitled(title: &str) -> SampleEvent {
    SampleEvent::TitleUpdated(title.to_owned())
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

async fn waiting(pool: &SqlitePool) -> Vec<String> {
    sqlx::query_scalar("SELECT routing_key FROM outbox WHERE published_at IS NULL ORDER BY entry")
        .fetch_all(pool)
        .await
        .expect("reading the outbox should succeed")
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

#[tokio::test(flavor = "multi_thread")]
async fn two_writers_at_the_same_version_cannot_both_win() {
    let bench = OnSqlite::setup().await;
    let aggregate = AggregateId::from("sample_contested");
    a_started_stream(bench.store(), &aggregate).await;

    let store = Arc::new(SqliteEventStore::new(
        bench.pool.clone(),
        codec(),
        message_for,
    ));
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
        "a lost race is a stale version, not a busy database: {refused:?}"
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
    let bench = OnSqlite::setup().await;
    let aggregate = AggregateId::from("sample_half");
    a_started_stream(bench.store(), &aggregate).await;

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
        waiting(&bench.pool).await,
        vec!["sample.created"],
        "only the stream's start announced: the outbox row shares its append's transaction"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn a_backend_that_cannot_be_reached_says_so() {
    let bench = OnSqlite::setup().await;
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
        "{reading:?}"
    );
    assert!(
        matches!(appending, Err(StoreError::Backend { .. })),
        "{appending:?}"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn an_event_written_in_a_shape_nothing_can_read_is_a_backend_failure() {
    let bench = OnSqlite::setup().await;
    let aggregate = AggregateId::from("sample_unreadable");
    a_started_stream(bench.store(), &aggregate).await;

    sqlx::query(
        "INSERT INTO events
            (aggregate, kind, version, name, body, body_version, agent, occurred_at, is_snapshot)
         VALUES (?1, ?2, 2, 'CREATED', ?3, 0, 'system', '1970-01-01T00:00:00.000000000Z', 0)",
    )
    .bind(aggregate.as_str())
    .bind(SAMPLE.as_str())
    .bind(nonsense().to_string())
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
    let bench = OnSqlite::setup().await;
    let aggregate = AggregateId::from("sample_columns");
    a_started_stream(bench.store(), &aggregate).await;

    let (name, shape, agent, occurred_at): (String, i64, String, String) = sqlx::query_as(
        "SELECT name, body_version, agent, occurred_at FROM events WHERE aggregate = ?1 AND version = 1",
    )
    .bind(aggregate.as_str())
    .fetch_one(&bench.pool)
    .await
    .expect("the row we just wrote should be there");

    assert_eq!(name, a_creation().name().as_str());
    assert_eq!(shape as u64, a_creation().version().count());
    assert_eq!(agent, "system");
    assert_eq!(
        occurred_at, "2025-09-27T19:06:40.123456789Z",
        "a file an author owns should be readable without the app"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn a_codec_is_all_a_second_kind_of_event_needs() {
    let fixture = SqliteFixture::setup();
    let store: SqliteEventStore<SampleEvent> = SqliteEventStore::new(
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

    assert_eq!(found[0].event, SampleEvent::Deleted);

    fixture.cleanup().await;
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
async fn laying_the_schema_down_twice_changes_nothing() {
    let fixture = SqliteFixture::setup();
    let pool = ready_for("again", &fixture).await;

    crate::sqlite::migrations()
        .run(&pool)
        .await
        .expect("a schema already laid down should be left alone");

    a_started_stream(
        &SqliteEventStore::new(pool.clone(), codec(), message_for),
        &AggregateId::from("sample_again"),
    )
    .await;

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

#[tokio::test]
async fn an_append_leaves_a_message_waiting_for_every_publishable_event() {
    let bench = OnSqlite::setup().await;
    let aggregate = AggregateId::from("sample_announced");

    bench
        .store()
        .append(
            &aggregate,
            SAMPLE,
            Version::ZERO,
            &[
                at(&aggregate, 1, a_creation()),
                at(&aggregate, 2, retitled("Second")),
                at(&aggregate, 3, SampleEvent::Corrected("Quietly".to_owned())),
            ],
        )
        .await
        .expect("appending should succeed");

    assert_eq!(
        waiting(&bench.pool).await,
        vec!["sample.created", "sample.title_updated"],
        "a correction publishes nothing, so it leaves nothing waiting"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn a_refused_append_leaves_no_message_waiting() {
    let bench = OnSqlite::setup().await;
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
    assert!(waiting(&bench.pool).await.is_empty());

    bench.cleanup().await;
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
    .bind((codec().body)(&retitled("Theirs")).to_string())
    .execute(&mut *other)
    .await
    .expect("the other writer writes");

    let store = Arc::new(SqliteEventStore::new(
        bench.pool.clone(),
        codec(),
        message_for,
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

#[tokio::test]
async fn a_version_below_zero_is_a_backend_failure_not_a_huge_version() {
    let bench = OnSqlite::setup().await;
    let aggregate = AggregateId::from("sample_negative");
    a_started_stream(bench.store(), &aggregate).await;

    sqlx::query("UPDATE events SET version = -3, is_snapshot = 1 WHERE aggregate = ?1")
        .bind(aggregate.as_str())
        .execute(&bench.pool)
        .await
        .expect("editing the file by hand should succeed");

    let found = bench.store().latest_snapshot(&aggregate, SAMPLE).await;

    assert!(
        matches!(found, Err(StoreError::Backend { .. })),
        "-3 read as a u64 is eighteen quintillion, a version nothing could ever have written: {found:?}"
    );

    bench.cleanup().await;
}
