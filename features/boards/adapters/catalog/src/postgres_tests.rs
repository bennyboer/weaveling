use std::time::{Duration, Instant};

use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;
use time::OffsetDateTime;
use transacting::{PostgresTransactions, Transactions};

use boards_core::{BoardCatalog, BoardId, IdeaLink, ProjectLink};

use crate::postgres;
use crate::postgres::PostgresBoardCatalog;
use crate::suite;
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    transactions: PostgresTransactions,
    store: PostgresBoardCatalog,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresBoardCatalog;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("boards").await;
        postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            transactions: PostgresTransactions::new(pool.clone()),
            store: PostgresBoardCatalog::new(pool.clone()),
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

crate::conformance_tests!(OnPostgres);

#[tokio::test]
async fn forgetting_a_board_leaves_no_half_of_it_behind() {
    let bench = OnPostgres::setup().await;
    let board = BoardId::generate(crate::suite::at(1_000));
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

    bench
        .committing()
        .forget(&board, suite::at(5_000))
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

#[tokio::test(flavor = "multi_thread")]
async fn remembering_and_forgetting_at_once_never_leaves_a_forgotten_board_standing() {
    let bench = OnPostgres::setup().await;
    let catalog = bench.committing();

    for nth in 0..40 {
        let summary = suite::a_summary(BoardId::generate(suite::at(1_000 + nth)), "project_1");
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
        "a remember racing a forget must never bring the forgotten board back"
    );
    bench.cleanup().await;
}

#[tokio::test]
async fn a_project_s_tombstone_waits_for_an_open_under_way() {
    let bench = OnPostgres::setup().await;
    let project = ProjectLink::from("project_loom");
    let mut opening = bench
        .transactions
        .begin()
        .await
        .expect("beginning should succeed");
    assert!(
        !bench
            .store
            .has_project_tombstone_within(&mut opening, &project)
            .await
            .expect("asking should succeed")
    );

    let (buried_at, opened_at) = tokio::join!(
        async {
            let mut burying = bench
                .transactions
                .begin()
                .await
                .expect("beginning should succeed");
            bench
                .store
                .lay_project_tombstone(&mut burying, &project, OffsetDateTime::UNIX_EPOCH)
                .await
                .expect("laying a tombstone should succeed");
            burying.finish().await.expect("committing should succeed");
            Instant::now()
        },
        async move {
            tokio::time::sleep(Duration::from_millis(300)).await;
            let opened_at = Instant::now();
            opening.finish().await.expect("committing should succeed");
            opened_at
        }
    );

    assert!(
        buried_at > opened_at,
        "the tombstone waits for an open that found none, so the sweep it asks for finds what the open claimed"
    );
    bench.cleanup().await;
}
