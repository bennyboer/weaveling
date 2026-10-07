use async_trait::async_trait;
use ideas_core::{IdeaCatalog, IdeaId, ProjectLink};
use sqlx::SqlitePool;
use test_harness::SqliteFixture;

use crate::sqlite::{SqliteIdeaCatalog, migrations};
use crate::suite::{Workbench, a_summary, at};

struct OnSqlite {
    fixture: SqliteFixture,
    pool: SqlitePool,
    store: SqliteIdeaCatalog,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Store = SqliteIdeaCatalog;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool: SqlitePool = fixture.create_database("ideas").await;
        migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            store: SqliteIdeaCatalog::new(pool.clone()),
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
async fn a_version_below_zero_is_refused_rather_than_read_as_a_huge_one() {
    let bench = OnSqlite::setup().await;
    bench
        .store
        .remember(&a_summary(
            IdeaId::generate(at(1_000)),
            "project_1",
            "The Loom",
        ))
        .await
        .expect("remembering should succeed");
    sqlx::query("UPDATE idea_summaries SET version = -3")
        .execute(&bench.pool)
        .await
        .expect("editing the file by hand should succeed");

    let found = bench
        .store
        .in_project(&ProjectLink::from("project_1"))
        .await;

    assert!(found.is_err(), "{found:?}");

    bench.cleanup().await;
}
