use assets_core::{AccountLimits, CatalogError};
use async_trait::async_trait;
use eventsourcing::AccountId;
use transacting::Transaction;
use transacting::sqlite::connection;

use crate::written::{size_written, unreachable};

const LIMIT_OF: &str = "SELECT most FROM asset_account_limits WHERE account = ?1";

const GIVE: &str = "
    INSERT INTO asset_account_limits (account, most) VALUES (?1, ?2)
    ON CONFLICT (account) DO UPDATE SET most = excluded.most
";

const TAKE_BACK: &str = "DELETE FROM asset_account_limits WHERE account = ?1";

/// The limits the operator gave single accounts, on Sqlite, beside the assets in the catalog's own schema.
#[derive(Debug, Default)]
pub struct SqliteAccountLimits;

impl SqliteAccountLimits {
    pub fn new() -> Self {
        Self
    }
}

#[async_trait]
impl AccountLimits for SqliteAccountLimits {
    async fn limit_of(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<Option<u64>, CatalogError> {
        sqlx::query_scalar::<_, i64>(LIMIT_OF)
            .bind(account.to_string())
            .fetch_optional(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?
            .map(|most| u64::try_from(most).map_err(unreachable))
            .transpose()
    }

    async fn give(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        most: u64,
    ) -> Result<(), CatalogError> {
        sqlx::query(GIVE)
            .bind(account.to_string())
            .bind(size_written(most)?)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn take_back(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), CatalogError> {
        sqlx::query(TAKE_BACK)
            .bind(account.to_string())
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use sqlx::SqlitePool;
    use test_harness::SqliteFixture;
    use transacting::{SqliteTransactions, Transactions};

    use super::*;
    use crate::account_limits_suite::Workbench;
    use crate::sqlite;

    struct OnSqlite {
        fixture: SqliteFixture,
        store: SqliteAccountLimits,
        transactions: SqliteTransactions,
    }

    #[async_trait]
    impl Workbench for OnSqlite {
        type Store = SqliteAccountLimits;

        async fn setup() -> Self {
            let fixture = SqliteFixture::setup();
            let pool: SqlitePool = fixture.create_database("assets").await;
            sqlite::migrations()
                .run(&pool)
                .await
                .expect("the schema should lay down in an empty file");

            Self {
                fixture,
                store: SqliteAccountLimits::new(),
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

    crate::account_limits_conformance_tests!(OnSqlite);
}
