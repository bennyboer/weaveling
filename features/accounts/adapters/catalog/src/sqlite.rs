use std::error::Error;

use accounts_core::{
    AccountCatalog, AccountId, AccountName, AccountSummary, CatalogError, EmailAddress,
};
use async_trait::async_trait;
use clock::text;
use eventsourcing::Version;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use crate::locks;

const REMEMBER: &str = "
    INSERT INTO account_summaries
        (account, version, name, email, verified, operator, signed_up_at, locks)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8)
    ON CONFLICT (account) DO UPDATE
    SET version      = excluded.version,
        name         = excluded.name,
        email        = excluded.email,
        verified     = excluded.verified,
        operator     = excluded.operator,
        signed_up_at = excluded.signed_up_at,
        locks        = excluded.locks
    WHERE account_summaries.version < excluded.version
";

const LISTED_AFTER: &str = "
    SELECT account, version, name, email, verified, operator, signed_up_at, locks
    FROM account_summaries
    WHERE ?1 IS NULL OR account < ?1
    ORDER BY account DESC
    LIMIT ?2
";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations/sqlite")
}

pub struct SqliteAccountCatalog {
    pool: SqlitePool,
}

impl SqliteAccountCatalog {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

fn to_summary(row: &SqliteRow) -> Result<AccountSummary, CatalogError> {
    let account: String = row.try_get("account").map_err(unreachable)?;
    let version: i64 = row.try_get("version").map_err(unreachable)?;
    let name: String = row.try_get("name").map_err(unreachable)?;
    let email: String = row.try_get("email").map_err(unreachable)?;
    let signed_up_at: String = row.try_get("signed_up_at").map_err(unreachable)?;
    let kept_locks: String = row.try_get("locks").map_err(unreachable)?;

    Ok(AccountSummary {
        id: account.parse().map_err(unreachable)?,
        version: Version::of(u64::try_from(version).map_err(unreachable)?),
        name: AccountName::new(&name).map_err(unreachable)?,
        email: EmailAddress::new(&email).map_err(unreachable)?,
        verified: row.try_get("verified").map_err(unreachable)?,
        operator: row.try_get("operator").map_err(unreachable)?,
        signed_up_at: text::read(&signed_up_at).ok_or_else(|| {
            CatalogError::Backend(
                format!("signed_up_at holds {signed_up_at:?}, which is no time").into(),
            )
        })?,
        locks: locks::read(&kept_locks)?,
    })
}

#[async_trait]
impl AccountCatalog for SqliteAccountCatalog {
    async fn remember(&self, summary: &AccountSummary) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(i64::try_from(summary.version.count()).map_err(unreachable)?)
            .bind(summary.name.as_str())
            .bind(summary.email.to_string())
            .bind(summary.verified)
            .bind(summary.operator)
            .bind(text::written(summary.signed_up_at))
            .bind(locks::written(&summary.locks))
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn listed_after(
        &self,
        after: Option<&AccountId>,
        at_most: usize,
    ) -> Result<Vec<AccountSummary>, CatalogError> {
        let found = sqlx::query(LISTED_AFTER)
            .bind(after.map(ToString::to_string))
            .bind(i64::try_from(at_most).map_err(unreachable)?)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found.iter().map(to_summary).collect()
    }
}
