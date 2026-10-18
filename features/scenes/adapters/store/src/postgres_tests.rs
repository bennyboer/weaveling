use std::sync::Arc;

use async_trait::async_trait;
use clock::FixedClock;
use messaging::Message;
use outbox::{Outbox, PostgresOutbox};
use scenes_core::{SceneId, StoreError};
use sqlx::PgPool;
use test_harness::PostgresFixture;

use crate::postgres::{PostgresSceneStore, migrations};
use crate::suite::{Heard, Tables, Workbench, at, message_for};

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    store: PostgresSceneStore,
    heard: Arc<Heard>,
}

struct Compacting(OnPostgres);

fn a_clock() -> Arc<FixedClock> {
    Arc::new(FixedClock::new(at(2_000)))
}

fn enqueuing(store: PostgresSceneStore) -> PostgresSceneStore {
    store.enqueuing(a_clock(), message_for)
}

async fn relayed(pool: &PgPool, heard: &Arc<Heard>) -> Vec<Message> {
    PostgresOutbox::new(pool.clone(), heard.clone(), a_clock())
        .deliver(100)
        .await
        .expect("the outbox should deliver");

    heard.messages()
}

async fn a_migrated_schema() -> (PostgresFixture, PgPool) {
    let fixture = PostgresFixture::setup().await;
    let pool = fixture.create_schema("scenes").await;
    outbox::postgres::migrations()
        .run(&pool)
        .await
        .expect("the outbox should lay down in an empty namespace");
    migrations()
        .run(&pool)
        .await
        .expect("the scene schema should lay down in an empty namespace");

    (fixture, pool)
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresSceneStore;

    async fn setup() -> Self {
        let (fixture, pool) = a_migrated_schema().await;

        Self {
            fixture,
            store: enqueuing(PostgresSceneStore::new(pool.clone())),
            pool,
            heard: Arc::new(Heard::default()),
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
    type Store = PostgresSceneStore;

    async fn setup() -> Self {
        let (fixture, pool) = a_migrated_schema().await;

        Self(OnPostgres {
            fixture,
            store: enqueuing(PostgresSceneStore::compacting_after(pool.clone(), 1)),
            pool,
            heard: Arc::new(Heard::default()),
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
impl Tables for OnPostgres {
    fn on_the_same_tables(&self, compacting_after: i64) -> PostgresSceneStore {
        PostgresSceneStore::compacting_after(self.pool.clone(), compacting_after)
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
    crate::suite::conformance_tests!(super::OnPostgres);
}

mod tables {
    crate::suite::table_tests!(super::OnPostgres);
}

mod compacting {
    crate::suite::conformance_tests!(super::Compacting);
}

#[tokio::test]
async fn walking_a_project_comes_out_of_the_index() {
    let bench = OnPostgres::setup().await;
    let mut written = Vec::new();
    for nth in 1..=2_000 {
        written.push(format!("scene_{nth:0>19}"));
    }

    sqlx::query(
        "INSERT INTO scenes (scene, project, title)
         SELECT held, 'project_' || (ordinality % 8), ''
         FROM unnest($1::text[]) WITH ORDINALITY AS held",
    )
    .bind(&written)
    .execute(&bench.pool)
    .await
    .expect("seeding should succeed");

    sqlx::query("ANALYZE scenes")
        .execute(&bench.pool)
        .await
        .expect("analysing should succeed");

    let plan: Vec<String> = sqlx::query_scalar(
        "EXPLAIN SELECT scene
         FROM scenes
         WHERE project = $1 AND scene > $2
         ORDER BY scene
         LIMIT $3",
    )
    .bind("project_3")
    .bind("scene_0000000000000000100")
    .bind(100_i64)
    .fetch_all(&bench.pool)
    .await
    .expect("explaining should succeed");
    let plan = plan.join("\n");

    assert!(
        plan.contains("scenes_by_project"),
        "deleting a project sweeps its prose one batch at a time, so the batch must come out \
         of the index rather than a scan that grows with every book ever written: {plan}"
    );
    assert!(
        !plan.contains("Seq Scan"),
        "a plan change here is silent — the sweep keeps working and only gets slower: {plan}"
    );
    assert!(
        !plan.contains("Filter:"),
        "both halves belong in the index condition — a filter means rows are read and then \
         thrown away, which is the cost the cursor exists to avoid: {plan}"
    );

    bench.cleanup().await;
}
