use async_trait::async_trait;
use projects_core::{ProjectCatalog, ProjectId};
use sqlx::PgPool;
use test_harness::PostgresFixture;

use crate::postgres::{PostgresProjectCatalog, migrations};
use crate::suite::{Workbench, a_summary, at};

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    store: PostgresProjectCatalog,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresProjectCatalog;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("projects").await;
        migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            store: PostgresProjectCatalog::new(pool.clone()),
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
async fn the_id_column_sorts_bytewise_whatever_the_database_locale_is() {
    let bench = OnPostgres::setup().await;

    let collation: Option<String> = sqlx::query_scalar(
        "SELECT collation_name::text
         FROM information_schema.columns
         WHERE table_schema = $1 AND table_name = 'project_summaries' AND column_name = 'project'",
    )
    .bind(bench.fixture.schema_of("projects"))
    .fetch_one(&bench.pool)
    .await
    .expect("reading the catalog should succeed");

    assert_eq!(
        collation.as_deref(),
        Some("C"),
        "the id orders the listing, so it must sort bytewise — base62 ids only sort by age \
         under the C collation, and a glibc locale puts 'a' before 'A'"
    );

    bench.cleanup().await;
}

#[tokio::test]
async fn a_version_below_zero_is_refused_rather_than_read_as_a_huge_one() {
    let bench = OnPostgres::setup().await;
    bench
        .store
        .remember(&a_summary(
            ProjectId::generate(at(1_000)),
            "The Silent Loom",
        ))
        .await
        .expect("remembering should succeed");
    sqlx::query("UPDATE project_summaries SET version = -3")
        .execute(&bench.pool)
        .await
        .expect("editing the table by hand should succeed");

    let found = bench.store.all().await;

    assert!(found.is_err(), "{found:?}");

    bench.cleanup().await;
}
