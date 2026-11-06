use async_trait::async_trait;
use registry::{PostgresRegistry, Registry};
use sqlx::PgPool;
use test_harness::PostgresFixture;
use time::OffsetDateTime;
use transacting::{Failure, PostgresTransactions, Transactions, Tried};

use outline_core::{Attachment, KIND, OutlineCatalog, OutlineId, ProjectLink};

use crate::postgres;
use crate::postgres::PostgresOutlineCatalog;
use crate::suite;
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    transactions: PostgresTransactions,
    store: PostgresOutlineCatalog,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresOutlineCatalog;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("outline").await;
        postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            transactions: PostgresTransactions::new(pool.clone()),
            store: PostgresOutlineCatalog::new(pool.clone()),
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
async fn forgetting_an_outline_leaves_no_half_of_it_behind() {
    // given: an outline catalogued and indexed
    let bench = OnPostgres::setup().await;
    let outline = OutlineId::generate(clock::at(1_000));
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
        .forget(&outline, clock::at(5_000))
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
    let bench = OnPostgres::setup().await;
    let catalog = bench.committing();

    // when: outlines are remembered and forgotten at once
    for nth in 0..40 {
        let summary = suite::a_summary(OutlineId::generate(clock::at(1_000 + nth)), "project_1");
        let (remembered, forgotten) = tokio::join!(
            catalog.remember(&summary),
            catalog.forget(&summary.id, clock::at(5_000)),
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

#[tokio::test]
async fn an_open_that_found_no_tombstone_loses_to_one_laid_and_swept_before_it_commits() {
    // given: an open that found no tombstone and claimed the project, not yet committed
    let bench = OnPostgres::setup().await;
    registry::postgres::migrations()
        .run(&bench.pool)
        .await
        .expect("the registry should lay down beside the catalog");
    let registry = PostgresRegistry::new(bench.pool.clone());
    let project = ProjectLink::from("project_loom");
    let outline = OutlineId::generate(clock::at(1_000));
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
            &outline.to_string(),
        )
        .await
        .expect("claiming should succeed");

    // when: the project's tombstone is laid
    transacting::retrying!(&bench.transactions, |burying| {
        bench
            .store
            .lay_project_tombstone(burying, &project, OffsetDateTime::UNIX_EPOCH)
            .await
            .map_err(Tried::Refused)
    })
    .expect("laying a tombstone should succeed");
    // and: the sweep reads who holds the project
    let swept = registry
        .holder(KIND.as_str(), project.as_str())
        .await
        .expect("the sweep should read the claims");

    // then: it finds nobody
    assert_eq!(
        swept, None,
        "the sweep cannot see a claim not committed yet"
    );
    // and: the open loses the race when it commits
    assert!(
        matches!(opening.finish().await, Err(Failure::LostRace)),
        "the open read no tombstone and the sweep read no claim, so the open, between the two, \
         loses and is tried again, then finding the tombstone"
    );
    bench.cleanup().await;
}
