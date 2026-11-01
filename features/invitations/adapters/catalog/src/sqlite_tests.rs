use std::sync::Arc;

use async_trait::async_trait;
use invitations_core::{CatalogError, InvitationCatalog, InvitationId};
use sqlx::SqlitePool;
use test_harness::SqliteFixture;
use transacting::{SqliteTransactions, Transactions};

use crate::sqlite;
use crate::sqlite::SqliteInvitationCatalog;
use crate::suite;
use crate::suite::Workbench;

struct OnSqlite {
    fixture: SqliteFixture,
    pool: SqlitePool,
    transactions: SqliteTransactions,
    store: SqliteInvitationCatalog,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Store = SqliteInvitationCatalog;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool = fixture.create_database("invitations").await;
        sqlite::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            transactions: SqliteTransactions::new(pool.clone()),
            store: SqliteInvitationCatalog::new(pool.clone()),
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

crate::conformance_tests!(OnSqlite);

#[tokio::test(flavor = "multi_thread")]
async fn remembering_and_forgetting_at_once_never_leaves_a_forgotten_invitation_standing() {
    let bench = OnSqlite::setup().await;
    let catalog = Arc::new(SqliteInvitationCatalog::new(bench.pool.clone()));
    let transactions = Arc::new(SqliteTransactions::new(bench.pool.clone()));

    for nth in 0..40 {
        let summary = suite::a_summary(
            InvitationId::generate(suite::at(1_000 + nth)),
            "ada@example.com",
        );
        let (remembering, forgetting) = (catalog.clone(), catalog.clone());
        let (remembered_in, forgotten_in) = (transactions.clone(), transactions.clone());
        let id = summary.id;
        let (remembered, forgotten) = tokio::join!(
            tokio::spawn(async move {
                let mut transaction = remembered_in
                    .begin()
                    .await
                    .expect("beginning should succeed");
                remembering.remember(&mut transaction, &summary).await?;
                transaction
                    .finish()
                    .await
                    .expect("finishing should succeed");
                Ok::<_, CatalogError>(())
            }),
            tokio::spawn(async move {
                let mut transaction = forgotten_in
                    .begin()
                    .await
                    .expect("beginning should succeed");
                forgetting
                    .forget(&mut transaction, &id, suite::at(5_000))
                    .await?;
                transaction
                    .finish()
                    .await
                    .expect("finishing should succeed");
                Ok::<_, CatalogError>(())
            }),
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
