use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;

use crate::postgres::{PostgresPieceCatalog, migrations};
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    store: PostgresPieceCatalog,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresPieceCatalog;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("pieces").await;
        migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            store: PostgresPieceCatalog::new(pool.clone()),
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
async fn the_id_columns_sort_bytewise_whatever_the_database_locale_is() {
    let bench = OnPostgres::setup().await;

    let collations: Vec<(String, Option<String>)> = sqlx::query_as(
        "SELECT column_name::text, collation_name::text
         FROM information_schema.columns
         WHERE table_schema = $1 AND table_name = 'piece_summaries'
         ORDER BY column_name",
    )
    .bind(bench.fixture.schema_of("pieces"))
    .fetch_all(&bench.pool)
    .await
    .expect("reading the catalog should succeed");

    for named in ["piece", "project", "passage"] {
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
    let bench = OnPostgres::setup().await;
    let mut written = Vec::new();
    for nth in 1..=2_000 {
        written.push(format!("piece_{nth:0>22}"));
    }

    sqlx::query(
        "INSERT INTO piece_summaries (piece, version, project, title)
         SELECT held, 1, 'project_' || (ordinality % 8), 'A piece'
         FROM unnest($1::text[]) WITH ORDINALITY AS held",
    )
    .bind(&written)
    .execute(&bench.pool)
    .await
    .expect("seeding should succeed");

    sqlx::query("ANALYZE piece_summaries")
        .execute(&bench.pool)
        .await
        .expect("analysing should succeed");

    let plan: Vec<String> = sqlx::query_scalar(
        "EXPLAIN SELECT piece, version, project, title, passage
         FROM piece_summaries
         WHERE project = $1 AND piece > $2
         ORDER BY piece
         LIMIT $3",
    )
    .bind("project_3")
    .bind("piece_0000000000000000000100")
    .bind(100_i64)
    .fetch_all(&bench.pool)
    .await
    .expect("explaining should succeed");
    let plan = plan.join("\n");

    assert!(
        plan.contains("piece_summaries_by_project"),
        "a sweep walks a project one batch at a time, so the batch must come out of the \
         index rather than a scan that grows with the table: {plan}"
    );
    assert!(
        !plan.contains("Seq Scan"),
        "the index is (project, piece DESC) and the sweep wants ascending, which only works \
         because PostgreSQL walks a b-tree backwards; a plan change here is silent: {plan}"
    );
    assert!(
        !plan.contains("Filter:"),
        "both halves belong in the index condition — a filter means rows are read and then \
         thrown away, which is the cost the cursor exists to avoid: {plan}"
    );

    bench.cleanup().await;
}
