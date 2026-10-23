use std::sync::Arc;

use async_trait::async_trait;
use clock::FixedClock;
use messaging::{FakePublisher, Message};
use sqlx::PgPool;
use test_harness::PostgresFixture;
use time::OffsetDateTime;

use crate::postgres::{PostgresOutbox, enqueue};
use crate::suite;
use crate::suite::{Bench, Then, Workbench};

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    outbox: PostgresOutbox,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Outbox = PostgresOutbox;

    async fn setup(publisher: Arc<FakePublisher>, clock: Arc<FixedClock>) -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("outbox").await;
        crate::postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down");
        let outbox = PostgresOutbox::new(pool.clone(), publisher, clock);

        Self {
            fixture,
            pool,
            outbox,
        }
    }

    fn outbox(&self) -> &PostgresOutbox {
        &self.outbox
    }

    async fn enqueue(&self, messages: &[Message], then: Then) {
        let mut transaction = self.pool.begin().await.expect("a transaction opens");
        for message in messages {
            enqueue(&mut transaction, suite::from_a_scene(), message)
                .await
                .expect("enqueueing should succeed");
        }
        match then {
            Then::Commit => transaction.commit().await.expect("the transaction commits"),
            Then::RollBack => transaction
                .rollback()
                .await
                .expect("the transaction rolls back"),
        }
    }

    async fn published(&self) -> Vec<bool> {
        sqlx::query_scalar::<_, Option<OffsetDateTime>>(
            "SELECT published_at FROM outbox ORDER BY entry",
        )
        .fetch_all(&self.pool)
        .await
        .expect("reading the outbox should succeed")
        .iter()
        .map(Option::is_some)
        .collect()
    }

    async fn all_published_at(&self, when: OffsetDateTime) {
        sqlx::query("UPDATE outbox SET published_at = $1")
            .bind(when)
            .execute(&self.pool)
            .await
            .expect("ageing an entry should succeed");
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

crate::outbox_conformance_tests!(OnPostgres, in_transactions);

#[tokio::test]
async fn the_outbox_keeps_its_own_ledger_beside_whoever_owns_the_database() {
    let bench = Bench::<OnPostgres>::setup().await;

    let found: Vec<String> = sqlx::query_scalar(
        "SELECT tablename::text FROM pg_tables WHERE schemaname = $1 ORDER BY tablename",
    )
    .bind(bench.workbench.fixture.schema_of("outbox"))
    .fetch_all(&bench.workbench.pool)
    .await
    .expect("reading the catalog should succeed");

    assert_eq!(
        found,
        vec!["_sqlx_migrations_outbox", "outbox"],
        "a writer that is not event-sourced gets the outbox without an events table"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn the_schema_carries_the_indexes_the_queries_rely_on() {
    let bench = Bench::<OnPostgres>::setup().await;

    let found: Vec<String> = sqlx::query_scalar(
        "SELECT indexname::text FROM pg_indexes WHERE schemaname = $1 ORDER BY indexname",
    )
    .bind(bench.workbench.fixture.schema_of("outbox"))
    .fetch_all(&bench.workbench.pool)
    .await
    .expect("reading the catalog should succeed");

    assert_eq!(
        found,
        vec![
            "_sqlx_migrations_outbox_pkey",
            "outbox_pkey",
            "outbox_published",
            "outbox_waiting",
        ]
    );

    bench.cleanup().await;
}
