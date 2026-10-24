use std::sync::Arc;

use async_trait::async_trait;
use clock::FixedClock;
use clock::text;
use messaging::{FakePublisher, Message};
use sqlx::SqlitePool;
use test_harness::SqliteFixture;
use time::OffsetDateTime;

use crate::outbox::Outbox;
use crate::relaying::{Cadence, RelayTask};
use crate::sqlite::SqliteOutbox;
use crate::suite;
use crate::suite::{Bench, Then, Workbench};

struct OnSqlite {
    fixture: SqliteFixture,
    pool: SqlitePool,
    outbox: Arc<SqliteOutbox>,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Outbox = SqliteOutbox;

    async fn setup(publisher: Arc<FakePublisher>, clock: Arc<FixedClock>) -> Self {
        let fixture = SqliteFixture::setup();
        let pool = fixture.create_database("outbox").await;
        crate::sqlite::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down");
        let outbox = Arc::new(SqliteOutbox::new(pool.clone(), publisher, clock));

        Self {
            fixture,
            pool,
            outbox,
        }
    }

    fn outbox(&self) -> &SqliteOutbox {
        &self.outbox
    }

    async fn enqueue(&self, messages: &[Message], then: Then) {
        let mut transaction = self.pool.begin().await.expect("a transaction opens");
        for message in messages {
            crate::sqlite::enqueue(&mut transaction, suite::from_a_scene(), message)
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
        sqlx::query_scalar::<_, Option<String>>("SELECT published_at FROM outbox ORDER BY entry")
            .fetch_all(&self.pool)
            .await
            .expect("reading the outbox should succeed")
            .iter()
            .map(Option::is_some)
            .collect()
    }

    async fn all_published_at(&self, when: OffsetDateTime) {
        sqlx::query("UPDATE outbox SET published_at = ?1")
            .bind(text::written(when))
            .execute(&self.pool)
            .await
            .expect("ageing an entry should succeed");
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

crate::outbox_conformance_tests!(OnSqlite, in_transactions);

#[tokio::test]
async fn an_enqueued_message_wakes_the_relay() {
    let bench = Bench::<OnSqlite>::setup().await;
    let mut notifications = bench
        .workbench
        .outbox
        .notifications()
        .await
        .expect("asking for notifications is not a failure");
    bench.waiting(1).await;

    let woken = tokio::time::timeout(std::time::Duration::from_secs(1), notifications.wait()).await;

    assert!(
        woken.is_ok(),
        "a read model must not trail a write by a whole poll, or a view opened right after it \
         shows what was there before"
    );

    bench.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_relay_woken_before_the_write_commits_still_finds_the_message() {
    let bench = Bench::<OnSqlite>::setup().await;
    let outbox = bench.workbench.outbox.clone();
    let message = suite::a_link_message();
    let mut transaction = bench
        .workbench
        .pool
        .begin()
        .await
        .expect("a transaction opens");
    crate::sqlite::enqueue(&mut transaction, suite::from_a_scene(), &message)
        .await
        .expect("enqueueing should succeed");

    let delivering = tokio::spawn({
        let outbox = outbox.clone();
        async move { outbox.deliver(10).await }
    });
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    transaction.commit().await.expect("the transaction commits");
    delivering
        .await
        .expect("delivering should not panic")
        .expect("delivering should succeed");

    assert_eq!(
        bench.ids_heard(),
        [message.id],
        "the bell rings inside the writer's transaction, so the relay's claim waits for the \
         write lock and sees the message once it commits"
    );

    bench.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_started_relay_delivers_by_polling_alone() {
    let bench = Bench::<OnSqlite>::setup().await;
    let relay = RelayTask::started(
        bench.workbench.outbox.clone(),
        Cadence {
            deliver_every: std::time::Duration::from_millis(20),
            ..Cadence::default()
        },
    );

    bench.waiting(2).await;

    let mut delivered = false;
    for _ in 0..500 {
        if bench.ids_heard().len() == 2 {
            delivered = true;
            break;
        }
        tokio::time::sleep(std::time::Duration::from_millis(10)).await;
    }
    relay.stop().await;

    assert!(
        delivered,
        "with no notification, the poll alone has to carry every message"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn the_outbox_keeps_its_own_ledger_beside_whoever_owns_the_file() {
    let bench = Bench::<OnSqlite>::setup().await;

    let found: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(&bench.workbench.pool)
    .await
    .expect("reading the catalog should succeed");

    assert_eq!(found, vec!["_sqlx_migrations_outbox", "outbox"]);

    bench.cleanup().await;
}

#[tokio::test]
async fn the_schema_carries_the_indexes_the_queries_rely_on() {
    let bench = Bench::<OnSqlite>::setup().await;

    let found: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'index' AND sql IS NOT NULL ORDER BY name",
    )
    .fetch_all(&bench.workbench.pool)
    .await
    .expect("reading the catalog should succeed");

    assert_eq!(found, vec!["outbox_published", "outbox_waiting"]);

    bench.cleanup().await;
}
