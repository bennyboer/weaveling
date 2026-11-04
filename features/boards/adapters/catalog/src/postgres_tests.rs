use async_trait::async_trait;
use registry::{PostgresRegistry, Registry};
use sqlx::PgPool;
use test_harness::PostgresFixture;
use time::OffsetDateTime;
use transacting::{Failure, PostgresTransactions, Transactions, Tried};

use boards_core::{BoardCatalog, BoardId, IdeaLink, KIND, ProjectLink};

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
async fn an_open_that_found_no_tombstone_loses_to_one_laid_and_swept_before_it_commits() {
    let bench = OnPostgres::setup().await;
    registry::postgres::migrations()
        .run(&bench.pool)
        .await
        .expect("the registry should lay down beside the catalog");
    let registry = PostgresRegistry::new(bench.pool.clone());
    let project = ProjectLink::from("project_loom");
    let board = BoardId::generate(suite::at(1_000));
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
    registry
        .claim(
            &mut opening,
            KIND.as_str(),
            project.as_str(),
            &board.to_string(),
        )
        .await
        .expect("claiming should succeed");

    transacting::retrying!(&bench.transactions, |burying| {
        bench
            .store
            .lay_project_tombstone(burying, &project, OffsetDateTime::UNIX_EPOCH)
            .await
            .map_err(Tried::Refused)
    })
    .expect("laying a tombstone should succeed");
    let swept = registry
        .holder(KIND.as_str(), project.as_str())
        .await
        .expect("the sweep should read the claims");

    assert_eq!(
        swept, None,
        "the sweep cannot see a claim not committed yet"
    );
    assert!(
        matches!(opening.finish().await, Err(Failure::LostRace)),
        "the open read no tombstone and the sweep read no claim, so the open, between the two, \
         loses and is tried again, then finding the tombstone"
    );
    bench.cleanup().await;
}
