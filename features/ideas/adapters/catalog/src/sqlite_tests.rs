use async_trait::async_trait;
use ideas_core::{IdeaCatalog, IdeaId, ProjectLink};
use sqlx::SqlitePool;
use test_harness::SqliteFixture;

use crate::sqlite;
use crate::sqlite::SqliteIdeaCatalog;
use crate::suite;
use crate::suite::Workbench;

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
        sqlite::migrations()
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
        .remember(&suite::a_summary(
            IdeaId::generate(suite::at(1_000)),
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

#[tokio::test(flavor = "multi_thread")]
async fn remembering_and_forgetting_at_once_never_leaves_a_forgotten_idea_standing() {
    let bench = OnSqlite::setup().await;
    let catalog = bench.store();

    for nth in 0..40 {
        let summary = suite::a_summary(
            IdeaId::generate(suite::at(1_000 + nth)),
            "project_1",
            "The Loom",
        );
        let (remembered, forgotten) = tokio::join!(
            catalog.remember(&summary),
            catalog.forget(&summary.id, suite::at(5_000)),
        );
        remembered.expect("remembering should succeed");
        forgotten.expect("forgetting should succeed");
    }

    assert!(
        catalog
            .in_project(&ProjectLink::from("project_1"))
            .await
            .expect("listing should succeed")
            .is_empty(),
        "a remember racing a forget must never bring the forgotten idea back"
    );
    bench.cleanup().await;
}
