use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;

use boards_core::{BoardCatalog, BoardId, IdeaLink};

use crate::postgres::{PostgresBoardCatalog, migrations};
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    store: PostgresBoardCatalog,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresBoardCatalog;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("boards").await;
        migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            store: PostgresBoardCatalog::new(pool.clone()),
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
async fn forgetting_a_board_leaves_no_half_of_it_behind() {
    let bench = OnPostgres::setup().await;
    let board = BoardId::generate(crate::suite::at(1_000));
    bench
        .store()
        .remember(&crate::suite::a_summary(board, "project_1"))
        .await
        .expect("remembering should succeed");
    bench
        .store()
        .holds(board, &[IdeaLink::from("idea_1")])
        .await
        .expect("indexing should succeed");

    bench
        .store()
        .forget(&board)
        .await
        .expect("forgetting should succeed");

    let summaries: i64 = sqlx::query_scalar("SELECT count(*) FROM board_summaries")
        .fetch_one(&bench.pool)
        .await
        .expect("counting should succeed");
    let pins: i64 = sqlx::query_scalar("SELECT count(*) FROM board_ideas")
        .fetch_one(&bench.pool)
        .await
        .expect("counting should succeed");

    assert_eq!(
        (summaries, pins),
        (0, 0),
        "the summary and the index go in one transaction, so a crash between them cannot leave the \
         index naming a board that is gone"
    );

    bench.cleanup().await;
}
