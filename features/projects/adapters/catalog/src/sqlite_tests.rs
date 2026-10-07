use async_trait::async_trait;
use projects_core::{ProjectCatalog, ProjectId};
use sqlx::SqlitePool;
use test_harness::SqliteFixture;

use crate::sqlite::{SqliteProjectCatalog, migrations};
use crate::suite::{Workbench, a_summary, at};

struct OnSqlite {
    fixture: SqliteFixture,
    pool: SqlitePool,
    store: SqliteProjectCatalog,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Store = SqliteProjectCatalog;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool = fixture.create_database("projects").await;
        migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            store: SqliteProjectCatalog::new(pool.clone()),
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
            ProjectId::generate(at(1_000)),
            "The Silent Loom",
        ))
        .await
        .expect("remembering should succeed");
    sqlx::query("UPDATE project_summaries SET version = -3")
        .execute(&bench.pool)
        .await
        .expect("editing the file by hand should succeed");

    let found = bench.store.all().await;

    assert!(found.is_err(), "{found:?}");

    bench.cleanup().await;
}

#[tokio::test]
async fn a_time_nothing_can_read_is_refused_rather_than_guessed() {
    let bench = OnSqlite::setup().await;
    bench
        .store
        .remember(&a_summary(
            ProjectId::generate(at(1_000)),
            "The Silent Loom",
        ))
        .await
        .expect("remembering should succeed");
    sqlx::query("UPDATE project_summaries SET updated_at = 'last tuesday'")
        .execute(&bench.pool)
        .await
        .expect("editing the file by hand should succeed");

    let found = bench.store.all().await;

    assert!(found.is_err(), "{found:?}");

    bench.cleanup().await;
}
