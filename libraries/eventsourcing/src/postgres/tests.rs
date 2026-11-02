use std::sync::Arc;

use async_trait::async_trait;
use clock::FixedClock;
use sqlx::PgPool;
use test_harness::PostgresFixture;
use time::OffsetDateTime;
use transacting::{PostgresTransactions, Transactions};

use crate::agent::Agent;
use crate::aggregate::AggregateId;
use crate::codec::Codec;
use crate::postgres::PostgresEventStore;
use crate::service::EventSourcingService;
use crate::testing::sample;
use crate::testing::sample::{SAMPLE, Sample, SampleCommand, SampleEvent};
use crate::testing::stored;
use crate::testing::{Tables, Workbench};

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    transactions: PostgresTransactions,
    store: PostgresEventStore<SampleEvent>,
}

async fn ready_for(feature: &str, fixture: &PostgresFixture) -> PgPool {
    let pool = fixture.create_schema(feature).await;
    crate::postgres::migrations()
        .run(&pool)
        .await
        .expect("the event store's schema should lay down in an empty namespace");
    outbox::postgres::migrations()
        .run(&pool)
        .await
        .expect("the outbox an append writes into should lay down beside it");

    pool
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresEventStore<SampleEvent>;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool = ready_for("sample", &fixture).await;
        let store = PostgresEventStore::new(pool.clone(), stored::codec(), sample::message_for);

        Self {
            fixture,
            transactions: PostgresTransactions::new(pool.clone()),
            pool,
            store,
        }
    }

    fn store(&self) -> &Self::Store {
        &self.store
    }

    fn transactions(&self) -> &dyn Transactions {
        &self.transactions
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
impl Tables for OnPostgres {
    fn reading_with(&self, codec: Codec<SampleEvent>) -> Self::Store {
        PostgresEventStore::new(self.pool.clone(), codec, sample::message_for)
    }

    async fn close(&self) {
        self.pool.close().await;
    }

    async fn lay_the_schema_down_again(&self) {
        crate::postgres::migrations()
            .run(&self.pool)
            .await
            .expect("a schema already laid down should be left alone");
    }

    async fn write_unreadable_event(&self, aggregate: &AggregateId, version: u64) {
        sqlx::query(
            "INSERT INTO events
                (aggregate, kind, version, name, body, body_version, agent, occurred_at, is_snapshot)
             VALUES ($1, $2, $3, 'CREATED', $4, 0, 'system', NOW(), false)",
        )
        .bind(aggregate.as_str())
        .bind(SAMPLE.as_str())
        .bind(version as i64)
        .bind(stored::nonsense())
        .execute(&self.pool)
        .await
        .expect("writing a row by hand should succeed");
    }

    async fn push_versions_below_zero(&self, aggregate: &AggregateId) {
        sqlx::query("UPDATE events SET version = -3, is_snapshot = true WHERE aggregate = $1")
            .bind(aggregate.as_str())
            .execute(&self.pool)
            .await
            .expect("editing the table by hand should succeed");
    }

    async fn columns(&self, aggregate: &AggregateId, version: u64) -> (String, i64, String) {
        sqlx::query_as(
            "SELECT name, body_version, agent FROM events WHERE aggregate = $1 AND version = $2",
        )
        .bind(aggregate.as_str())
        .bind(version as i64)
        .fetch_one(&self.pool)
        .await
        .expect("the row just written should be there")
    }

    async fn versions_waiting(&self) -> Vec<Option<i64>> {
        sqlx::query_scalar("SELECT version FROM outbox WHERE published_at IS NULL ORDER BY entry")
            .fetch_all(&self.pool)
            .await
            .expect("reading the outbox should succeed")
    }
}

crate::conformance_tests!(OnPostgres);
crate::table_tests!(OnPostgres);

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
            "_sqlx_migrations_outbox_pkey",
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

#[tokio::test(flavor = "multi_thread")]
async fn commands_on_one_sample_in_one_transaction_need_no_other_connection() {
    let bench = OnPostgres::setup().await;
    let service: EventSourcingService<Sample> = EventSourcingService::new(
        Arc::new(PostgresEventStore::new(
            bench.pool.clone(),
            stored::codec(),
            sample::message_for,
        )),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );
    let aggregate = AggregateId::from("sample_on_one_connection");
    let mut transaction = bench
        .transactions
        .begin()
        .await
        .expect("beginning should succeed");
    let mut taken = Vec::new();
    for _ in 1..bench.pool.options().get_max_connections() {
        taken.push(
            bench
                .pool
                .acquire()
                .await
                .expect("the rest of the pool should be at hand"),
        );
    }

    tokio::time::timeout(std::time::Duration::from_secs(3), async {
        service
            .begin(
                &mut transaction,
                &aggregate,
                SampleCommand::Create {
                    title: "The Loom".to_owned(),
                    description: "A silent machine.".to_owned(),
                },
                &Agent::System,
            )
            .await
            .expect("beginning the sample should succeed");
        for title in ["Second", "Third"] {
            service
                .execute(
                    &mut transaction,
                    &aggregate,
                    SampleCommand::UpdateTitle(title.to_owned()),
                    &Agent::System,
                )
                .await
                .expect("a command after another in the same transaction reads what it appended");
        }
    })
    .await
    .expect("a command reads through the transaction it is given, needing no connection beside it");
    drop(taken);
    transaction
        .finish()
        .await
        .expect("finishing should succeed");

    assert_eq!(
        service
            .latest(&aggregate)
            .await
            .expect("the sample should be there")
            .state
            .title,
        "Third"
    );
    bench.cleanup().await;
}
