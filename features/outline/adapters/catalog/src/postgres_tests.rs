use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;

use outline_core::{Attachment, OutlineCatalog, OutlineId};

use crate::postgres::{PostgresOutlineCatalog, migrations};
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    store: PostgresOutlineCatalog,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresOutlineCatalog;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("outline").await;
        migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            store: PostgresOutlineCatalog::new(pool.clone()),
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

crate::conformance_tests!(OnPostgres);

#[tokio::test]
async fn forgetting_an_outline_leaves_no_half_of_it_behind() {
    let bench = OnPostgres::setup().await;
    let outline = OutlineId::generate(crate::suite::at(1_000));
    bench
        .store()
        .remember(&crate::suite::a_summary(outline, "project_1"))
        .await
        .expect("remembering should succeed");
    bench
        .store()
        .holds(outline, &[Attachment::scene("scene_1")])
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
