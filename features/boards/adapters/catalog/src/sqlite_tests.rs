use async_trait::async_trait;
use sqlx::SqlitePool;
use test_harness::SqliteFixture;

use boards_core::{BoardCatalog, BoardId, IdeaLink};

use crate::sqlite::{SqliteBoardCatalog, migrations};
use crate::suite::Workbench;

struct OnSqlite {
    fixture: SqliteFixture,
    pool: SqlitePool,
    store: SqliteBoardCatalog,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Store = SqliteBoardCatalog;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool: SqlitePool = fixture.create_database("boards").await;
        migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            store: SqliteBoardCatalog::new(pool.clone()),
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
async fn forgetting_a_board_leaves_no_half_of_it_behind() {
    let bench = OnSqlite::setup().await;
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
