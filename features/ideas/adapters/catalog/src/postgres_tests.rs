use async_trait::async_trait;
use ideas_core::{IdeaCatalog, IdeaId, ProjectLink};
use sqlx::PgPool;
use test_harness::PostgresFixture;
use time::OffsetDateTime;
use transacting::{Failure, PostgresTransactions, Transactions, Tried};

use crate::postgres;
use crate::postgres::PostgresIdeaCatalog;
use crate::suite;
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    transactions: PostgresTransactions,
    store: PostgresIdeaCatalog,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresIdeaCatalog;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("ideas").await;
        postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            transactions: PostgresTransactions::new(pool.clone()),
            store: PostgresIdeaCatalog::new(pool.clone()),
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
async fn the_id_columns_sort_bytewise_whatever_the_database_locale_is() {
    // given: a laid down schema
    let bench = OnPostgres::setup().await;

    // when: the collations of its columns are read
    let collations: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT column_name::text, collation_name::text
         FROM information_schema.columns
         WHERE table_schema = $1 AND table_name = 'idea_summaries'
         ORDER BY column_name",
    )
    .bind(bench.fixture.schema_of("ideas"))
    .fetch_all(&bench.pool)
    .await
    .expect("reading the catalog should succeed");

    // then: the id columns sort bytewise
    for named in ["idea", "project"] {
        let found = collations
            .iter()
            .find(|(column, _)| column == named)
            .map(|(_, collation)| collation.as_deref());

        assert_eq!(
            found,
            Some(Some("C")),
            "{named} orders the listing, so it must sort bytewise — base62 ids only \
             sort by age under the C collation, and a glibc locale puts 'a' before 'A'"
        );
    }

    bench.cleanup().await;
}

#[tokio::test]
async fn a_batch_is_read_straight_out_of_the_index() {
    // given: many ideas across a few projects
    let bench = OnPostgres::setup().await;
    let mut written = Vec::new();
    for nth in 1..=2_000 {
        written.push(format!("idea_{nth:0>22}"));
    }

    sqlx::query(
        "INSERT INTO idea_summaries (idea, version, project, title)
         SELECT held, 1, 'project_' || (ordinality % 8), 'An idea'
         FROM unnest($1::text[]) WITH ORDINALITY AS held",
    )
    .bind(&written)
    .execute(&bench.pool)
    .await
    .expect("seeding should succeed");

    // and: fresh statistics on them
    sqlx::query("ANALYZE idea_summaries")
        .execute(&bench.pool)
        .await
        .expect("analysing should succeed");

    // when: the plan for reading a batch is explained
    let plan: Vec<String> = sqlx::query_scalar(
        "EXPLAIN SELECT idea, version, project, title
         FROM idea_summaries
         WHERE project = $1 AND idea > $2
         ORDER BY idea
         LIMIT $3",
    )
    .bind("project_3")
    .bind("idea_0000000000000000000100")
    .bind(100_i64)
    .fetch_all(&bench.pool)
    .await
    .expect("explaining should succeed");
    let plan = plan.join("\n");

    // then: the batch comes out of the index
    assert!(
        plan.contains("idea_summaries_by_project"),
        "a sweep walks a project one batch at a time, so the batch must come out of the \
         index rather than a scan that grows with the table: {plan}"
    );
    // and: nothing is scanned
    assert!(
        !plan.contains("Seq Scan"),
        "the index is (project, idea DESC) and the sweep wants ascending, which only works \
         because PostgreSQL walks a b-tree backwards; a plan change here is silent: {plan}"
    );
    // and: nothing is filtered after reading
    assert!(
        !plan.contains("Filter:"),
        "both halves belong in the index condition — a filter means rows are read and then \
         thrown away, which is the cost the cursor exists to avoid: {plan}"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn a_version_below_zero_is_refused_rather_than_read_as_a_huge_one() {
    // given: a remembered idea
    let bench = OnPostgres::setup().await;
    bench
        .committing()
        .remember(&suite::a_summary(
            IdeaId::generate(clock::at(1_000)),
            "project_1",
            "The Loom",
        ))
        .await
        .expect("remembering should succeed");
    // and: its stored version set below zero
    sqlx::query("UPDATE idea_summaries SET version = -3")
        .execute(&bench.pool)
        .await
        .expect("editing the table by hand should succeed");

    // when: its project is listed
    let found = bench
        .store
        .in_project(&ProjectLink::from("project_1"))
        .await;

    // then: the listing is refused
    assert!(found.is_err(), "{found:?}");

    bench.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn remembering_and_forgetting_at_once_never_leaves_a_forgotten_idea_standing() {
    // given: a catalog
    let bench = OnPostgres::setup().await;
    let catalog = bench.committing();

    // when: ideas are remembered and forgotten at once
    for nth in 0..40 {
        let summary = suite::a_summary(
            IdeaId::generate(clock::at(1_000 + nth)),
            "project_1",
            "The Loom",
        );
        let (remembered, forgotten) = tokio::join!(
            catalog.remember(&summary),
            catalog.forget(&summary.id, clock::at(5_000)),
        );
        remembered.expect("remembering should succeed");
        forgotten.expect("forgetting should succeed");
    }

    // then: none of them stands
    assert!(
        catalog
            .in_project(&ProjectLink::from("project_1"))
            .await
            .expect("listing should succeed")
            .is_empty(),
        "a remember racing a forget must never bring the forgotten idea back"
    );
    bench.cleanup().await;
}

#[tokio::test]
async fn a_cataloguing_that_found_no_tombstone_loses_to_one_laid_and_swept_before_it_commits() {
    // given: a cataloguing that found no tombstone for its project
    let bench = OnPostgres::setup().await;
    let project = ProjectLink::from("project_loom");
    let summary = suite::a_summary(
        IdeaId::generate(clock::at(1_000)),
        "project_loom",
        "The Loom",
    );
    let mut cataloguing = bench
        .transactions
        .begin()
        .await
        .expect("beginning should succeed");
    assert!(
        !bench
            .store
            .has_project_tombstone_within(&mut cataloguing, &project)
            .await
            .expect("asking should succeed")
    );
    bench
        .store
        .remember(&mut cataloguing, &summary)
        .await
        .expect("remembering should succeed");

    // when: a tombstone is laid for the project
    transacting::retrying!(&bench.transactions, |burying| {
        bench
            .store
            .lay_project_tombstone(burying, &project, OffsetDateTime::UNIX_EPOCH)
            .await
            .map_err(Tried::Refused)
    })
    .expect("laying a tombstone should succeed");
    // and: the project is swept before the cataloguing commits
    let swept = bench
        .store
        .in_project_after(&project, None, 10)
        .await
        .expect("the sweep should read the catalog");

    // then: the sweep finds nothing
    assert!(
        swept.is_empty(),
        "the sweep cannot see an idea not catalogued yet"
    );
    // and: the cataloguing loses its race
    assert!(
        matches!(cataloguing.finish().await, Err(Failure::LostRace)),
        "the cataloguing read no tombstone and the sweep read no idea, so the cataloguing, between \
         the two, loses and is tried again, then finding the tombstone and discarding the idea"
    );
    bench.cleanup().await;
}
