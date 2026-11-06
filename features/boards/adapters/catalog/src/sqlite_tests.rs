use async_trait::async_trait;
use sqlx::SqlitePool;
use test_harness::SqliteFixture;
use transacting::{SqliteTransactions, Transactions};

use boards_core::{BoardCatalog, BoardId, IdeaLink, ProjectLink};

use crate::sqlite;
use crate::sqlite::SqliteBoardCatalog;
use crate::suite;
use crate::suite::Workbench;

struct OnSqlite {
    fixture: SqliteFixture,
    pool: SqlitePool,
    transactions: SqliteTransactions,
    store: SqliteBoardCatalog,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Store = SqliteBoardCatalog;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool: SqlitePool = fixture.create_database("boards").await;
        sqlite::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            transactions: SqliteTransactions::new(pool.clone()),
            store: SqliteBoardCatalog::new(pool.clone()),
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
async fn forgetting_a_board_leaves_no_half_of_it_behind() {
    // given: a board holding an idea
    let bench = OnSqlite::setup().await;
    let board = BoardId::generate(clock::at(1_000));
    bench
        .committing()
        .remember(&crate::suite::a_summary(board, "project_1"))
        .await
        .expect("remembering should succeed");
    bench
        .committing()
        .holds(board, &[IdeaLink::from("idea_1")])
        .await
        .expect("indexing should succeed");

    // when: the board is forgotten
    bench
        .committing()
        .forget(&board, clock::at(5_000))
        .await
        .expect("forgetting should succeed");

    // then: neither its summary nor its pins are left
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

#[tokio::test(flavor = "multi_thread")]
async fn remembering_and_forgetting_at_once_never_leaves_a_forgotten_board_standing() {
    // given: a catalog
    let bench = OnSqlite::setup().await;
    let catalog = bench.committing();

    // when: boards are remembered and forgotten at once
    for nth in 0..40 {
        let summary = suite::a_summary(BoardId::generate(clock::at(1_000 + nth)), "project_1");
        let (remembered, forgotten) = tokio::join!(
            catalog.remember(&summary),
            catalog.forget(&summary.id, clock::at(5_000)),
        );
        remembered.expect("remembering should succeed");
        forgotten.expect("forgetting should succeed");
    }

    // then: none of them is listed
    assert!(
        catalog
            .in_project(&ProjectLink::from("project_1"))
            .await
            .expect("listing should succeed")
            .is_empty(),
        "a remember racing a forget must never bring the forgotten board back"
    );
    bench.cleanup().await;
}
