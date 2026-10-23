use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;

use crate::postgres;
use crate::postgres::PostgresInvitationCatalog;
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    store: PostgresInvitationCatalog,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresInvitationCatalog;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("invitations").await;
        postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            store: PostgresInvitationCatalog::new(pool.clone()),
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
         WHERE table_schema = $1 AND table_name = 'invitation_summaries' AND column_name = 'invitation'",
    )
    .bind(bench.fixture.schema_of("invitations"))
    .fetch_one(&bench.pool)
    .await
    .expect("reading the list should succeed");

    assert_eq!(
        collation.as_deref(),
        Some("C"),
        "the id orders the listing and pages it, so it must sort bytewise"
    );

    bench.cleanup().await;
}
