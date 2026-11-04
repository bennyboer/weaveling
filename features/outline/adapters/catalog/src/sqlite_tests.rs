use async_trait::async_trait;
use sqlx::SqlitePool;
use test_harness::SqliteFixture;
use transacting::{SqliteTransactions, Transactions};

use outline_core::{Attachment, OutlineCatalog, OutlineId, ProjectLink};

use crate::sqlite;
use crate::sqlite::SqliteOutlineCatalog;
use crate::suite;
use crate::suite::Workbench;

struct OnSqlite {
    fixture: SqliteFixture,
    pool: SqlitePool,
    transactions: SqliteTransactions,
    store: SqliteOutlineCatalog,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Store = SqliteOutlineCatalog;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool: SqlitePool = fixture.create_database("outline").await;
        sqlite::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            transactions: SqliteTransactions::new(pool.clone()),
            store: SqliteOutlineCatalog::new(pool.clone()),
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
async fn forgetting_an_outline_leaves_no_half_of_it_behind() {
    // given: an outline catalogued and indexed
    let bench = OnSqlite::setup().await;
    let outline = OutlineId::generate(crate::suite::at(1_000));
    bench
        .committing()
        .remember(&crate::suite::a_summary(outline, "project_1"))
        .await
        .expect("remembering should succeed");
    bench
        .committing()
        .holds(outline, &[Attachment::scene("scene_1")])
        .await
        .expect("indexing should succeed");

    // when: it is forgotten
    bench
        .committing()
        .forget(&outline, suite::at(5_000))
        .await
        .expect("forgetting should succeed");

    // then: neither its summary nor its index is left
    let summaries: i64 = sqlx::query_scalar("SELECT count(*) FROM outline_summaries")
        .fetch_one(&bench.pool)
        .await
        .expect("counting should succeed");
    let pins: i64 = sqlx::query_scalar("SELECT count(*) FROM outline_attachments")
        .fetch_one(&bench.pool)
        .await
        .expect("counting should succeed");

    assert_eq!(
        (summaries, pins),
        (0, 0),
        "the summary and the index go in one transaction, so a crash between them cannot leave the \
         index naming a outline that is gone"
    );

    bench.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn remembering_and_forgetting_at_once_never_leaves_a_forgotten_outline_standing() {
    // given: a catalog
    let bench = OnSqlite::setup().await;
    let catalog = bench.committing();

    // when: outlines are remembered and forgotten at once
    for nth in 0..40 {
        let summary = suite::a_summary(OutlineId::generate(suite::at(1_000 + nth)), "project_1");
        let (remembered, forgotten) = tokio::join!(
            catalog.remember(&summary),
            catalog.forget(&summary.id, suite::at(5_000)),
        );
        remembered.expect("remembering should succeed");
        forgotten.expect("forgetting should succeed");
    }

    // then: none is left standing
    assert!(
        catalog
            .in_project(&ProjectLink::from("project_1"))
            .await
            .expect("listing should succeed")
            .is_empty(),
        "a remember racing a forget must never bring the forgotten outline back"
    );
    bench.cleanup().await;
}
