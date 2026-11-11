use async_trait::async_trait;
use sqlx::SqlitePool;
use test_harness::SqliteFixture;

use transacting::{SqliteTransactions, Transactions};

use crate::sqlite;
use crate::sqlite::SqliteBelongings;
use crate::suite::Workbench;

struct OnSqlite {
    fixture: SqliteFixture,
    store: SqliteBelongings,
    transactions: SqliteTransactions,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Store = SqliteBelongings;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool: SqlitePool = fixture.create_database("belonging").await;
        sqlite::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            store: SqliteBelongings::new(pool.clone()),
            transactions: SqliteTransactions::new(pool),
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
