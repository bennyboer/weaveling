use std::sync::Arc;

use async_trait::async_trait;
use invitations_core::{InvitationCatalog, InvitationId};
use sqlx::PgPool;
use test_harness::PostgresFixture;

use crate::postgres;
use crate::postgres::PostgresInvitationCatalog;
use crate::suite;
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

#[tokio::test(flavor = "multi_thread")]
async fn remembering_and_forgetting_at_once_never_leaves_a_forgotten_invitation_standing() {
    let bench = OnPostgres::setup().await;
    let catalog = Arc::new(PostgresInvitationCatalog::new(bench.pool.clone()));

    for nth in 0..40 {
        let summary = suite::a_summary(
            InvitationId::generate(suite::at(1_000 + nth)),
            "ada@example.com",
        );
        let (remembering, forgetting) = (catalog.clone(), catalog.clone());
        let id = summary.id;
        let (remembered, forgotten) = tokio::join!(
            tokio::spawn(async move { remembering.remember(&summary).await }),
            tokio::spawn(async move { forgetting.forget(&id, suite::at(5_000)).await }),
        );
        remembered
            .expect("remembering should not panic")
            .expect("remembering should succeed");
        forgotten
            .expect("forgetting should not panic")
            .expect("forgetting should succeed");
    }

    assert!(
        catalog
            .listed_after(None, 100)
            .await
            .expect("listing should succeed")
            .is_empty(),
        "a remember racing a forget must never bring the forgotten invitation back"
    );
    bench.cleanup().await;
}
