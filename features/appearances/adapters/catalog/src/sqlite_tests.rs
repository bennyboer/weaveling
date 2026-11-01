use async_trait::async_trait;
use sqlx::SqlitePool;
use test_harness::SqliteFixture;
use transacting::{SqliteTransactions, Transactions};

use crate::sqlite;
use crate::sqlite::SqliteAppearanceCatalog;
use crate::suite::Workbench;

struct OnSqlite {
    fixture: SqliteFixture,
    transactions: SqliteTransactions,
    catalog: SqliteAppearanceCatalog,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Catalog = SqliteAppearanceCatalog;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool: SqlitePool = fixture.create_database("appearances").await;
        sqlite::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            transactions: SqliteTransactions::new(pool.clone()),
            catalog: SqliteAppearanceCatalog::new(pool),
        }
    }

    fn catalog(&self) -> &Self::Catalog {
        &self.catalog
    }

    fn transactions(&self) -> &dyn Transactions {
        &self.transactions
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

crate::suite::conformance_tests!(OnSqlite);
