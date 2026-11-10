use std::error::Error;

use accounts_core::{
    AccountCatalog, AccountId, AccountName, AccountSummary, CatalogError, EmailAddress, InProject,
};
use async_trait::async_trait;
use clock::text;
use eventsourcing::Version;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use transacting::Transaction;
use transacting::sqlite::connection;

use crate::locks;

const REMEMBER: &str = "
    INSERT INTO account_summaries
        (account, version, name, email, verified, operator, signed_up_at, locks, second_factor)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9)
    ON CONFLICT (account) DO UPDATE
    SET version      = excluded.version,
        name         = excluded.name,
        email        = excluded.email,
        verified     = excluded.verified,
        operator     = excluded.operator,
        signed_up_at = excluded.signed_up_at,
        locks        = excluded.locks,
        second_factor = excluded.second_factor
    WHERE account_summaries.version < excluded.version
";

const LISTED_AFTER: &str = "
    SELECT account, version, name, email, verified, operator, signed_up_at, second_factor, locks
    FROM account_summaries
    WHERE ?1 IS NULL OR account < ?1
    ORDER BY account DESC
    LIMIT ?2
";

const OWNER: &str = "owner";
const MEMBER: &str = "member";

const REMEMBER_HOLDING: &str = "
    INSERT INTO project_holdings (project, account, version, held)
    VALUES (?1, ?2, ?3, ?4)
    ON CONFLICT (project, account) DO UPDATE
    SET version = excluded.version, held = excluded.held
    WHERE project_holdings.version < excluded.version
";

const REMEMBER_DISBANDED: &str = "
    INSERT INTO disbanded_projects (project, version)
    VALUES (?1, ?2)
    ON CONFLICT (project) DO NOTHING
";

const PROJECTS_TO_HAND_OVER: &str = "
    SELECT mine.project
    FROM project_holdings mine
    WHERE mine.account = ?1 AND mine.held = 'owner'
      AND NOT EXISTS (SELECT 1 FROM disbanded_projects gone WHERE gone.project = mine.project)
      AND NOT EXISTS (
          SELECT 1 FROM trashed_projects t WHERE t.project = mine.project AND t.in_the_trash = 1
      )
      AND NOT EXISTS (
          SELECT 1 FROM project_holdings other
          WHERE other.project = mine.project AND other.account <> mine.account
            AND other.held = 'owner'
      )
      AND EXISTS (
          SELECT 1 FROM project_holdings other
          WHERE other.project = mine.project AND other.account <> mine.account
            AND other.held IS NOT NULL
      )
    ORDER BY mine.project
";

const REMEMBER_TRASH: &str = "
    INSERT INTO trashed_projects (project, version, in_the_trash)
    VALUES (?1, ?2, ?3)
    ON CONFLICT (project) DO UPDATE
    SET version = excluded.version, in_the_trash = excluded.in_the_trash
    WHERE trashed_projects.version < excluded.version
";

const FORGET_TRASH: &str = "DELETE FROM trashed_projects WHERE project = ?1";

const IN_THE_TRASH: &str = "SELECT in_the_trash FROM trashed_projects WHERE project = ?1";

const LEDGER: &str = "_sqlx_migrations_account_summaries";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/sqlite");
    laying.dangerous_set_table_name(LEDGER);

    laying
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
        second_factor: row.try_get("second_factor").map_err(unreachable)?,
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
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &AccountSummary,
    ) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(i64::try_from(summary.version.count()).map_err(unreachable)?)
            .bind(summary.name.as_str())
            .bind(summary.email.to_string())
            .bind(summary.verified)
            .bind(summary.operator)
            .bind(text::written(summary.signed_up_at))
            .bind(locks::written(&summary.locks))
            .bind(summary.second_factor)
            .execute(connection(transaction).map_err(unreachable)?)
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

    async fn remember_holding(
        &self,
        transaction: &mut Transaction,
        project: &str,
        account: &AccountId,
        version: Version,
        held: Option<InProject>,
    ) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER_HOLDING)
            .bind(project)
            .bind(account.to_string())
            .bind(i64::try_from(version.count()).map_err(unreachable)?)
            .bind(held_named(held))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn remember_disbanded(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
    ) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER_DISBANDED)
            .bind(project)
            .bind(i64::try_from(version.count()).map_err(unreachable)?)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn remember_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER_TRASH)
            .bind(project)
            .bind(i64::try_from(version.count()).map_err(unreachable)?)
            .bind(in_the_trash)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget_trash(
        &self,
        transaction: &mut Transaction,
        project: &str,
    ) -> Result<(), CatalogError> {
        sqlx::query(FORGET_TRASH)
            .bind(project)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn is_in_the_trash(&self, project: &str) -> Result<bool, CatalogError> {
        let kept: Option<bool> = sqlx::query_scalar(IN_THE_TRASH)
            .bind(project)
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(kept.unwrap_or(false))
    }

    async fn projects_to_hand_over(
        &self,
        account: &AccountId,
    ) -> Result<Vec<String>, CatalogError> {
        sqlx::query_scalar(PROJECTS_TO_HAND_OVER)
            .bind(account.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)
    }
}

fn held_named(held: Option<InProject>) -> Option<&'static str> {
    held.map(|held| match held {
        InProject::Owner => OWNER,
        InProject::Member => MEMBER,
    })
}

#[cfg(test)]
mod ledger_tests {
    use super::*;

    #[test]
    fn the_catalog_keeps_a_ledger_of_its_own_beside_the_secrets_one() {
        // expect: the catalog's migrations keep their own ledger
        assert_eq!(migrations().table_name, LEDGER);
        // and: not the default one
        assert_ne!(
            migrations().table_name,
            Migrator::DEFAULT.table_name,
            "the secrets already lay down a 0001 in the accounts database under the default ledger"
        );
    }
}
