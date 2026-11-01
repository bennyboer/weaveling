use async_trait::async_trait;
use projects_core::{ProjectCatalog, ProjectId};
use sqlx::SqlitePool;
use test_harness::SqliteFixture;
use transacting::{SqliteTransactions, Transactions};

use crate::sqlite;
use crate::sqlite::SqliteProjectCatalog;
use crate::suite;
use crate::suite::Workbench;

struct OnSqlite {
    fixture: SqliteFixture,
    pool: SqlitePool,
    transactions: SqliteTransactions,
    store: SqliteProjectCatalog,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Store = SqliteProjectCatalog;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool = fixture.create_database("projects").await;
        sqlite::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            transactions: SqliteTransactions::new(pool.clone()),
            store: SqliteProjectCatalog::new(pool.clone()),
            pool,
        }
    }

    fn store(&self) -> &Self::Store {
        &self.store
    }

    fn transactions(&self) -> &dyn Transactions {
        &self.transactions
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
        .committing()
        .remember(&suite::a_summary(
            ProjectId::generate(suite::at(1_000)),
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
        .committing()
        .remember(&suite::a_summary(
            ProjectId::generate(suite::at(1_000)),
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

#[tokio::test(flavor = "multi_thread")]
async fn remembering_and_forgetting_at_once_never_leaves_a_forgotten_project_standing() {
    let bench = OnSqlite::setup().await;
    let catalog = bench.committing();

    for nth in 0..40 {
        let summary =
            suite::a_summary(ProjectId::generate(suite::at(1_000 + nth)), "Working Title");
        let (remembered, forgotten) = tokio::join!(
            catalog.remember(&summary),
            catalog.forget(&summary.id, suite::at(5_000)),
        );
        remembered.expect("remembering should succeed");
        forgotten.expect("forgetting should succeed");
    }

    assert!(
        catalog
            .all()
            .await
            .expect("listing should succeed")
            .is_empty(),
        "a remember racing a forget must never bring the forgotten project back"
    );
    bench.cleanup().await;
}
