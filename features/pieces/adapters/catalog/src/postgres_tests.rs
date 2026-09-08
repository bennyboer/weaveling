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
