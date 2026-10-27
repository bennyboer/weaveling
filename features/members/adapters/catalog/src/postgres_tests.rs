use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;

use crate::postgres;
use crate::postgres::PostgresMembershipCatalog;
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    store: PostgresMembershipCatalog,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresMembershipCatalog;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("members").await;
        postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            store: PostgresMembershipCatalog::new(pool.clone()),
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
        "SELECT table_name::text || '.' || column_name::text, collation_name::text
         FROM information_schema.columns
         WHERE table_schema = $1 AND column_name IN ('project', 'account')
         ORDER BY 1",
    )
    .bind(bench.fixture.schema_of("members"))
    .fetch_all(&bench.pool)
    .await
    .expect("reading the columns should succeed");

    assert_eq!(
        collations,
        vec![
            ("memberships.account".to_owned(), Some("C".to_owned())),
            ("memberships.project".to_owned(), Some("C".to_owned())),
            ("people.account".to_owned(), Some("C".to_owned())),
            (
                "project_member_lists.project".to_owned(),
                Some("C".to_owned())
            ),
        ],
        "the project orders an account's memberships, so ids must sort bytewise"
    );

    bench.cleanup().await;
}
