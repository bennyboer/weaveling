use std::sync::Arc;

use async_trait::async_trait;
use clock::FixedClock;
use messaging::{FakePublisher, Message};
use outbox::{Outbox, SqliteOutbox};
use scenes_core::{SceneId, StoreError};
use sqlx::{Row, SqlitePool};
use test_harness::SqliteFixture;

use crate::sqlite;
use crate::sqlite::SqliteSceneStore;
use crate::suite;
use crate::suite::{Tables, Workbench};

struct OnSqlite {
    fixture: SqliteFixture,
    pool: SqlitePool,
    store: SqliteSceneStore,
    heard: Arc<FakePublisher>,
}

struct Compacting(OnSqlite);

fn a_clock() -> Arc<FixedClock> {
    Arc::new(FixedClock::new(suite::at(2_000)))
}

fn enqueuing(store: SqliteSceneStore) -> SqliteSceneStore {
    store.enqueuing(a_clock(), suite::message_for)
}

async fn relayed(pool: &SqlitePool, heard: &Arc<FakePublisher>) -> Vec<Message> {
    SqliteOutbox::new(pool.clone(), heard.clone(), a_clock())
        .deliver(100)
        .await
        .expect("the outbox should deliver");

    heard.heard()
}

async fn a_migrated_schema() -> (SqliteFixture, SqlitePool) {
    let fixture = SqliteFixture::setup();
    let pool = fixture.create_database("scenes").await;
    outbox::sqlite::migrations()
        .run(&pool)
        .await
        .expect("the outbox should lay down in an empty file");
    sqlite::migrations()
        .run(&pool)
        .await
        .expect("the scene schema should lay down in an empty file");

    (fixture, pool)
}

#[async_trait]
impl Workbench for OnSqlite {
    type Store = SqliteSceneStore;

    async fn setup() -> Self {
        let (fixture, pool) = a_migrated_schema().await;

        Self {
            fixture,
            store: enqueuing(SqliteSceneStore::new(pool.clone())),
            pool,
            heard: Arc::new(FakePublisher::default()),
        }
    }

    fn store(&self) -> &Self::Store {
        &self.store
    }

    async fn enqueued(&self) -> Vec<Message> {
        relayed(&self.pool, &self.heard).await
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

#[async_trait]
impl Workbench for Compacting {
    type Store = SqliteSceneStore;

    async fn setup() -> Self {
        let (fixture, pool) = a_migrated_schema().await;

        Self(OnSqlite {
            fixture,
            store: enqueuing(SqliteSceneStore::compacting_after(pool.clone(), 1)),
            pool,
            heard: Arc::new(FakePublisher::default()),
        })
    }

    fn store(&self) -> &Self::Store {
        &self.0.store
    }

    async fn enqueued(&self) -> Vec<Message> {
        relayed(&self.0.pool, &self.0.heard).await
    }

    async fn cleanup(self) {
        self.0.cleanup().await;
    }
}

#[async_trait]
impl Tables for OnSqlite {
    fn on_the_same_tables(&self, compacting_after: i64) -> SqliteSceneStore {
        SqliteSceneStore::compacting_after(self.pool.clone(), compacting_after)
    }

    async fn compact(&self, id: SceneId) -> Result<bool, StoreError> {
        self.store.compact(id).await
    }

    async fn rows(&self) -> Vec<bool> {
        sqlx::query_scalar("SELECT is_snapshot FROM scene_updates ORDER BY seq")
            .fetch_all(&self.pool)
            .await
            .expect("reading the parts should succeed")
    }

    async fn count(&self, query: &'static str) -> i64 {
        sqlx::query_scalar(query)
            .fetch_one(&self.pool)
            .await
            .expect("counting should succeed")
    }
}

mod appending {
    crate::suite::conformance_tests!(super::OnSqlite);
}

mod tables {
    crate::suite::table_tests!(super::OnSqlite);
}

mod compacting {
    crate::suite::conformance_tests!(super::Compacting);
}

#[tokio::test]
async fn walking_a_project_comes_out_of_the_index() {
    let bench = OnSqlite::setup().await;

    let plan: Vec<String> = sqlx::query(
        "EXPLAIN QUERY PLAN
         SELECT scene
         FROM scenes
         WHERE project = ?1 AND scene > ?2
         ORDER BY scene
         LIMIT ?3",
    )
    .bind("project_3")
    .bind("scene_0000000000000000100")
    .bind(100_i64)
    .fetch_all(&bench.pool)
    .await
    .expect("explaining should succeed")
    .iter()
    .map(|step| step.get::<String, _>("detail"))
    .collect();
    let plan = plan.join(
        "
",
    );

    assert!(
        plan.contains("USING COVERING INDEX scenes_by_project")
            || plan.contains("USING INDEX scenes_by_project"),
        "deleting a project sweeps its prose one batch at a time, so the batch must come out of the index: {plan}"
    );
    assert!(
        !plan.contains("SCAN scenes") && !plan.contains("TEMP B-TREE"),
        "a scan or a sort here is silent; the sweep keeps working and only gets slower: {plan}"
    );

    bench.cleanup().await;
}
