use std::time::Duration;

use async_trait::async_trait;
use sqlx::SqlitePool;
use test_harness::SqliteFixture;

use crate::deliveries::sqlite::{SqliteDeliveries, migrations};
use crate::deliveries::suite::{Workbench, a_listener, a_message, at};
use crate::delivering::Deliveries;

struct OnSqlite {
    fixture: SqliteFixture,
    pool: SqlitePool,
    store: SqliteDeliveries,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Store = SqliteDeliveries;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool = fixture.create_database("messaging").await;
        migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            store: SqliteDeliveries::new(pool.clone()),
            pool,
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

#[tokio::test]
async fn an_enqueued_delivery_wakes_whoever_is_waiting_for_one() {
    let bench = OnSqlite::setup().await;
    let mut notifications = bench
        .store()
        .notifications()
        .await
        .expect("asking for notifications is not a failure");

    bench
        .store()
        .enqueue(&a_listener("catalogue-idea"), &a_message("idea.captured"))
        .await
        .expect("enqueuing should succeed");
    let woken = tokio::time::timeout(Duration::from_secs(1), notifications.wait()).await;

    assert!(
        woken.is_ok(),
        "enqueue does its own write, so unlike the outbox it can wake the consumer the moment it lands"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn giving_up_moves_a_delivery_rather_than_copying_it() {
    let bench = OnSqlite::setup().await;
    bench
        .store()
        .enqueue(&a_listener("catalogue-idea"), &a_message("idea.captured"))
        .await
        .expect("enqueuing should succeed");
    let claimed = bench
        .store()
        .claim_due(at(1_000), 16)
        .await
        .expect("claiming should succeed");

    bench
        .store()
        .give_up(claimed[0].id, "it refused five times")
        .await
        .expect("giving up should succeed");

    assert_eq!(
        bench
            .store()
            .waiting()
            .await
            .expect("counting should succeed"),
        0
    );
    assert_eq!(
        bench
            .store()
            .dead_letters()
            .await
            .expect("reading should succeed")
            .len(),
        1,
        "a dead letter is the delivery it was, not a second copy beside a live one"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn the_schema_keeps_its_own_ledger_and_the_index_claiming_relies_on() {
    let bench = OnSqlite::setup().await;

    let tables: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'table' AND name NOT LIKE 'sqlite_%' ORDER BY name",
    )
    .fetch_all(&bench.pool)
    .await
    .expect("reading the catalog should succeed");
    let indexes: Vec<String> = sqlx::query_scalar(
        "SELECT name FROM sqlite_master WHERE type = 'index' AND sql IS NOT NULL ORDER BY name",
    )
    .fetch_all(&bench.pool)
    .await
    .expect("reading the catalog should succeed");

    assert_eq!(
        tables,
        vec!["_sqlx_migrations_deliveries", "dead_letters", "deliveries"]
    );
    assert_eq!(indexes, vec!["deliveries_due"]);

    bench.cleanup().await;
}
