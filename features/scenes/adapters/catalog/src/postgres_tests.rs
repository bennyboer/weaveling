use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;
use transacting::{PostgresTransactions, Transactions};

use crate::postgres;
use crate::postgres::{PostgresQuotationRestylingTasks, PostgresSceneCatalog};
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    transactions: PostgresTransactions,
    catalog: PostgresSceneCatalog,
    tasks: PostgresQuotationRestylingTasks,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Catalog = PostgresSceneCatalog;
    type Tasks = PostgresQuotationRestylingTasks;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("scenes").await;
        postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            transactions: PostgresTransactions::new(pool.clone()),
            catalog: PostgresSceneCatalog::new(pool.clone()),
            tasks: PostgresQuotationRestylingTasks::new(pool),
        }
    }

    fn catalog(&self) -> &Self::Catalog {
        &self.catalog
    }

    fn tasks(&self) -> &Self::Tasks {
        &self.tasks
    }

    fn transactions(&self) -> &dyn Transactions {
        &self.transactions
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

crate::suite::conformance_tests!(OnPostgres);
