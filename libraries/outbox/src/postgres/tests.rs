use std::sync::Arc;

use async_trait::async_trait;
use clock::FixedClock;
use messaging::{FakePublisher, Message};
use sqlx::PgPool;
use test_harness::PostgresFixture;
use time::OffsetDateTime;
use transacting::{PostgresTransactions, Transactions};

use crate::outbox::{Origin, Outbox};
use crate::postgres::PostgresOutbox;
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
        let mut transaction = PostgresTransactions::new(self.pool.clone())
            .begin()
            .await
            .expect("a transaction opens");
        for message in messages {
            self.outbox
                .enqueue_within(&mut transaction, suite::from_a_scene(), message)
                .await
                .expect("enqueueing should succeed");
        }
        match then {
            Then::Commit => transaction.finish().await.expect("the transaction commits"),
            Then::RollBack => drop(transaction),
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

crate::outbox_conformance_tests!(OnPostgres);

#[tokio::test]
async fn the_outbox_keeps_its_own_ledger_beside_whoever_owns_the_database() {
    // given: an outbox laid down on postgres
    let bench = Bench::<OnPostgres>::setup().await;

    // when: the tables of its namespace are listed
    let found: Vec<String> = sqlx::query_scalar(
        "SELECT tablename::text FROM pg_tables WHERE schemaname = $1 ORDER BY tablename",
    )
    .bind(bench.workbench.fixture.schema_of("outbox"))
    .fetch_all(&bench.workbench.pool)
    .await
    .expect("reading the catalog should succeed");

    // then: they are its own ledger and its migrations alone
    assert_eq!(
        found,
        vec!["_sqlx_migrations_outbox", "outbox"],
        "a writer that is not event-sourced gets the outbox without an events table"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn the_schema_carries_the_indexes_the_queries_rely_on() {
    // given: an outbox laid down on postgres
    let bench = Bench::<OnPostgres>::setup().await;

    // when: the indexes of its namespace are listed
    let found: Vec<String> = sqlx::query_scalar(
        "SELECT indexname::text FROM pg_indexes WHERE schemaname = $1 ORDER BY indexname",
    )
    .bind(bench.workbench.fixture.schema_of("outbox"))
    .fetch_all(&bench.workbench.pool)
    .await
    .expect("reading the catalog should succeed");

    // then: they are those its queries rely on
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

#[tokio::test]
async fn an_event_s_version_is_kept_beside_its_message_and_an_unversioned_one_keeps_none() {
    // given: an outbox on postgres
    let bench = Bench::<OnPostgres>::setup().await;
    // when: a message of a versioned event and one of no version are enqueued
    let mut transaction = PostgresTransactions::new(bench.workbench.pool.clone())
        .begin()
        .await
        .expect("a transaction opens");
    for origin in [
        Origin {
            aggregate: "idea_1",
            kind: "idea",
            version: Some(3),
        },
        Origin::unversioned("board_1", "board"),
    ] {
        bench
            .workbench
            .outbox
            .enqueue_within(&mut transaction, origin, &suite::a_link_message())
            .await
            .expect("enqueueing should succeed");
    }
    transaction.finish().await.expect("the transaction commits");

    // then: the event's version is kept and the other keeps none
    let kept: Vec<Option<i64>> = sqlx::query_scalar("SELECT version FROM outbox ORDER BY entry")
        .fetch_all(&bench.workbench.pool)
        .await
        .expect("reading the outbox should succeed");

    assert_eq!(
        kept,
        vec![Some(3), None],
        "a follow-up is no event and has no version to tell"
    );

    bench.cleanup().await;
}
