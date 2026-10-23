use std::error::Error;

use accounts_core::{
    AccountCatalog, AccountId, AccountName, AccountSummary, CatalogError, EmailAddress,
};
use async_trait::async_trait;
use eventsourcing::Version;
use sqlx::migrate::Migrator;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

use crate::locks;

const REMEMBER: &str = "
    INSERT INTO account_summaries
        (account, version, name, email, verified, operator, signed_up_at, locks)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8::JSONB)
    ON CONFLICT (account) DO UPDATE
    SET version      = EXCLUDED.version,
        name         = EXCLUDED.name,
        email        = EXCLUDED.email,
        verified     = EXCLUDED.verified,
        operator     = EXCLUDED.operator,
        signed_up_at = EXCLUDED.signed_up_at,
        locks        = EXCLUDED.locks
    WHERE account_summaries.version < EXCLUDED.version
";

const LISTED_AFTER: &str = "
    SELECT account, version, name, email, verified, operator, signed_up_at, locks::TEXT AS locks
    FROM account_summaries
    WHERE $1::TEXT IS NULL OR account < $1
    ORDER BY account DESC
    LIMIT $2
";

const LEDGER: &str = "_sqlx_migrations_account_summaries";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/postgres");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct PostgresAccountCatalog {
    pool: PgPool,
}

impl PostgresAccountCatalog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

fn to_summary(row: &PgRow) -> Result<AccountSummary, CatalogError> {
    let account: String = row.try_get("account").map_err(unreachable)?;
    let version: i64 = row.try_get("version").map_err(unreachable)?;
    let name: String = row.try_get("name").map_err(unreachable)?;
    let email: String = row.try_get("email").map_err(unreachable)?;
    let signed_up_at: OffsetDateTime = row.try_get("signed_up_at").map_err(unreachable)?;
    let kept_locks: String = row.try_get("locks").map_err(unreachable)?;

    Ok(AccountSummary {
        id: account.parse().map_err(unreachable)?,
        version: Version::of(u64::try_from(version).map_err(unreachable)?),
        name: AccountName::new(&name).map_err(unreachable)?,
        email: EmailAddress::new(&email).map_err(unreachable)?,
        verified: row.try_get("verified").map_err(unreachable)?,
        operator: row.try_get("operator").map_err(unreachable)?,
        signed_up_at,
        locks: locks::read(&kept_locks)?,
    })
}

#[async_trait]
impl AccountCatalog for PostgresAccountCatalog {
    async fn remember(&self, summary: &AccountSummary) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(i64::try_from(summary.version.count()).map_err(unreachable)?)
            .bind(summary.name.as_str())
            .bind(summary.email.to_string())
            .bind(summary.verified)
            .bind(summary.operator)
            .bind(summary.signed_up_at)
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

#[cfg(test)]
mod ledger_tests {
    use super::*;

    #[test]
    fn the_catalog_keeps_a_ledger_of_its_own_beside_the_secrets_one() {
        assert_eq!(migrations().table_name, LEDGER);
        assert_ne!(
            migrations().table_name,
            Migrator::DEFAULT.table_name,
            "the secrets already lay down a 0001 in the accounts database under the default ledger"
        );
    }
}
