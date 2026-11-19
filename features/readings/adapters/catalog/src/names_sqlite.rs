use std::collections::HashMap;

use async_trait::async_trait;
use eventsourcing::AccountId;
use readings_core::{AccountNameCatalog, CatalogError};
use sqlx::{Row, SqlitePool};
use time::OffsetDateTime;
use transacting::Transaction;
use transacting::sqlite::connection;

use crate::sqlite::{in_microseconds, unreachable};

const REMEMBER: &str = "
    INSERT INTO account_names (account, name)
    SELECT ?1, ?2
    WHERE NOT EXISTS (SELECT 1 FROM account_name_tombstones WHERE account = ?1)
    ON CONFLICT (account) DO UPDATE SET name = excluded.name
";

const LAY_TOMBSTONE: &str =
    "INSERT INTO account_name_tombstones (account, laid_at) VALUES (?1, ?2) ON CONFLICT DO NOTHING";

const FORGET: &str = "DELETE FROM account_names WHERE account = ?1";

const FORGET_OLD_TOMBSTONES: &str = "
    DELETE FROM account_name_tombstones WHERE account IN (
        SELECT account FROM account_name_tombstones
        WHERE laid_at < ?1
        ORDER BY laid_at, account
        LIMIT ?2
    )
";

const NAMES_OF: &str =
    "SELECT account, name FROM account_names WHERE account IN (SELECT value FROM json_each(?1))";

pub struct SqliteAccountNameCatalog {
    pool: SqlitePool,
}

impl SqliteAccountNameCatalog {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl AccountNameCatalog for SqliteAccountNameCatalog {
    async fn remember_name(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        name: &str,
    ) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER)
            .bind(account.as_str())
            .bind(name)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget_name(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        sqlx::query(LAY_TOMBSTONE)
            .bind(account.as_str())
            .bind(in_microseconds(at))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;
        sqlx::query(FORGET)
            .bind(account.as_str())
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        Ok(sqlx::query(FORGET_OLD_TOMBSTONES)
            .bind(in_microseconds(before))
            .bind(i64::try_from(at_most).unwrap_or(i64::MAX))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?
            .rows_affected())
    }

    async fn names_of(
        &self,
        accounts: &[AccountId],
    ) -> Result<HashMap<AccountId, String>, CatalogError> {
        let asked: Vec<&str> = accounts.iter().map(AccountId::as_str).collect();
        let asked = serde_json::to_string(&asked).expect("accounts are plain data");

        sqlx::query(NAMES_OF)
            .bind(asked)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
            .iter()
            .map(|row| {
                let account: String = row.try_get("account").map_err(unreachable)?;
                let name: String = row.try_get("name").map_err(unreachable)?;

                Ok((AccountId::from(account), name))
            })
            .collect()
    }
}
