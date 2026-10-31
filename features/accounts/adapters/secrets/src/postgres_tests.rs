use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;
use transacting::{PostgresTransactions, Transactions};

use crate::postgres;
use crate::postgres::PostgresSecrets;
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    store: PostgresSecrets,
    transactions: PostgresTransactions,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresSecrets;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("accounts").await;
        postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            store: PostgresSecrets::new(pool.clone()),
            transactions: PostgresTransactions::new(pool),
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
