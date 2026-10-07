use async_trait::async_trait;
use sqlx::SqlitePool;
use test_harness::SqliteFixture;

use outline_core::{Attachment, OutlineCatalog, OutlineId};

use crate::sqlite::{SqliteOutlineCatalog, migrations};
use crate::suite::Workbench;

struct OnSqlite {
    fixture: SqliteFixture,
    pool: SqlitePool,
    store: SqliteOutlineCatalog,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Store = SqliteOutlineCatalog;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool: SqlitePool = fixture.create_database("outline").await;
        migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            store: SqliteOutlineCatalog::new(pool.clone()),
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
async fn forgetting_an_outline_leaves_no_half_of_it_behind() {
    let bench = OnSqlite::setup().await;
    let outline = OutlineId::generate(crate::suite::at(1_000));
    bench
        .store()
        .remember(&crate::suite::a_summary(outline, "project_1"))
        .await
        .expect("remembering should succeed");
    bench
        .store()
        .holds(outline, &[Attachment::passage("passage_1")])
        .await
        .expect("indexing should succeed");

    bench
        .store()
        .forget(&outline)
        .await
        .expect("forgetting should succeed");

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
