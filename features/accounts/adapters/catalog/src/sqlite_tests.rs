use async_trait::async_trait;
use test_harness::SqliteFixture;
use transacting::{SqliteTransactions, Transactions};

use crate::sqlite;
use crate::sqlite::SqliteAccountCatalog;
use crate::suite::Workbench;

struct OnSqlite {
    fixture: SqliteFixture,
    transactions: SqliteTransactions,
    store: SqliteAccountCatalog,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Store = SqliteAccountCatalog;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool = fixture.create_database("accounts").await;
        sqlite::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            transactions: SqliteTransactions::new(pool.clone()),
            store: SqliteAccountCatalog::new(pool),
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
