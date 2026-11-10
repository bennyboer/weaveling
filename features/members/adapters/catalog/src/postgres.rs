use std::error::Error;

use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use members_core::{
    CatalogError, Leaving, Membership, MembershipCatalog, Person, ProjectLink, ProjectMembers, Role,
};
use sqlx::migrate::Migrator;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use transacting::Transaction;
use transacting::postgres::connection;

use crate::role;

const CLAIM: &str = "
    INSERT INTO project_member_lists (project, version)
    VALUES ($1, $2)
    ON CONFLICT (project) DO UPDATE
    SET version = EXCLUDED.version
    WHERE project_member_lists.version < EXCLUDED.version
";

const CLEAR: &str = "DELETE FROM memberships WHERE project = $1";

const ADD: &str = "INSERT INTO memberships (project, account, role) VALUES ($1, $2, $3)";

const ROLE_OF: &str = "SELECT role FROM memberships WHERE project = $1 AND account = $2";

const MEMBERSHIPS_OF: &str = "
    SELECT project, role
    FROM memberships
    WHERE account = $1
    ORDER BY project
";

const FORGET_PERSON: &str = "DELETE FROM people WHERE account = $1";

const REMEMBER_PERSON: &str = "
    INSERT INTO people (account, name, email)
    VALUES ($1, $2, $3)
    ON CONFLICT (account) DO UPDATE
    SET name = EXCLUDED.name, email = EXCLUDED.email
";

const PEOPLE: &str = "
    SELECT account, name, email
    FROM people
    WHERE account = ANY($1)
    ORDER BY account
";

const REMEMBER_TRASH: &str = "
    INSERT INTO project_trash (project, version, in_the_trash)
    VALUES ($1, $2, $3)
    ON CONFLICT (project) DO UPDATE
    SET version = EXCLUDED.version, in_the_trash = EXCLUDED.in_the_trash
    WHERE project_trash.version < EXCLUDED.version
";

const FORGET_TRASH: &str = "DELETE FROM project_trash WHERE project = $1";

const IN_THE_TRASH: &str = "SELECT in_the_trash FROM project_trash WHERE project = $1";

const REMEMBER_LEAVING: &str = "
    INSERT INTO leaving_accounts (account, version, until, settled_at)
    VALUES ($1, $2, $3, $4)
    ON CONFLICT (account) DO UPDATE
    SET version = EXCLUDED.version, until = EXCLUDED.until, settled_at = EXCLUDED.settled_at
    WHERE leaving_accounts.version < EXCLUDED.version
";

const FORGET_SETTLED_LEAVING: &str = "
    DELETE FROM leaving_accounts WHERE account IN (
        SELECT account FROM leaving_accounts
        WHERE settled_at < $1
        ORDER BY settled_at, account
        LIMIT $2
    )
";

const LEAVING: &str = "
    SELECT account, until
    FROM leaving_accounts
    WHERE account = ANY($1) AND until IS NOT NULL
    ORDER BY account
";

const LEDGER: &str = "_sqlx_migrations_memberships";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/postgres");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct PostgresMembershipCatalog {
    pool: PgPool,
}

impl PostgresMembershipCatalog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl MembershipCatalog for PostgresMembershipCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        members: &ProjectMembers,
    ) -> Result<(), CatalogError> {
        let claimed = sqlx::query(CLAIM)
            .bind(members.project.as_str())
            .bind(i64::try_from(members.version.count()).map_err(unreachable)?)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        if claimed.rows_affected() > 0 {
            sqlx::query(CLEAR)
                .bind(members.project.as_str())
                .execute(connection(transaction).map_err(unreachable)?)
                .await
                .map_err(unreachable)?;
            for member in &members.members {
                sqlx::query(ADD)
                    .bind(members.project.as_str())
                    .bind(member.account.as_str())
                    .bind(role::written(member.role))
                    .execute(connection(transaction).map_err(unreachable)?)
                    .await
                    .map_err(unreachable)?;
            }
        }

        Ok(())
    }

    async fn role_of(
        &self,
        project: &ProjectLink,
        account: &AccountId,
    ) -> Result<Option<Role>, CatalogError> {
        let kept: Option<String> = sqlx::query_scalar(ROLE_OF)
            .bind(project.as_str())
            .bind(account.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?;

        kept.as_deref().map(role::read).transpose()
    }

    async fn memberships_of(&self, account: &AccountId) -> Result<Vec<Membership>, CatalogError> {
        let found = sqlx::query(MEMBERSHIPS_OF)
            .bind(account.as_str())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found
            .iter()
            .map(|row| {
                let project: String = row.try_get("project").map_err(unreachable)?;
                let kept: String = row.try_get("role").map_err(unreachable)?;

                Ok(Membership {
                    project: ProjectLink::from(project),
                    role: role::read(&kept)?,
                })
            })
            .collect()
    }

    async fn remember_person(
        &self,
        transaction: &mut Transaction,
        person: &Person,
    ) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER_PERSON)
            .bind(person.account.as_str())
            .bind(&person.name)
            .bind(&person.email)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget_person(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
    ) -> Result<(), CatalogError> {
        sqlx::query(FORGET_PERSON)
            .bind(account.as_str())
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn people(&self, accounts: &[AccountId]) -> Result<Vec<Person>, CatalogError> {
        let asked: Vec<&str> = accounts.iter().map(AccountId::as_str).collect();
        let found = sqlx::query(PEOPLE)
            .bind(asked)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found.iter().map(person_in).collect()
    }

    async fn remember_trash(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER_TRASH)
            .bind(project.as_str())
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
        project: &ProjectLink,
    ) -> Result<(), CatalogError> {
        sqlx::query(FORGET_TRASH)
            .bind(project.as_str())
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn is_in_the_trash(&self, project: &ProjectLink) -> Result<bool, CatalogError> {
        let kept: Option<bool> = sqlx::query_scalar(IN_THE_TRASH)
            .bind(project.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(kept.unwrap_or(false))
    }

    async fn remember_leaving(
        &self,
        transaction: &mut Transaction,
        account: &AccountId,
        version: Version,
        until: Option<OffsetDateTime>,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER_LEAVING)
            .bind(account.as_str())
            .bind(i64::try_from(version.count()).map_err(unreachable)?)
            .bind(until)
            .bind(until.is_none().then_some(at))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget_leaving_settled_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        Ok(sqlx::query(FORGET_SETTLED_LEAVING)
            .bind(before)
            .bind(i64::try_from(at_most).map_err(unreachable)?)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?
            .rows_affected())
    }

    async fn leaving(&self, accounts: &[AccountId]) -> Result<Vec<Leaving>, CatalogError> {
        let asked: Vec<&str> = accounts.iter().map(AccountId::as_str).collect();
        let rows = sqlx::query(LEAVING)
            .bind(&asked)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        rows.iter()
            .map(|row| {
                let account: String = row.try_get("account").map_err(unreachable)?;

                Ok(Leaving {
                    account: AccountId::from(account),
                    until: row.try_get("until").map_err(unreachable)?,
                })
            })
            .collect()
    }
}

fn person_in(row: &PgRow) -> Result<Person, CatalogError> {
    let account: String = row.try_get("account").map_err(unreachable)?;

    Ok(Person {
        account: AccountId::from(account),
        name: row.try_get("name").map_err(unreachable)?,
        email: row.try_get("email").map_err(unreachable)?,
    })
}

#[cfg(test)]
mod ledger_tests {
    use super::*;

    #[test]
    fn the_read_model_keeps_a_ledger_of_its_own_beside_the_events_one() {
        // expect: its migrations are kept in a ledger of their own
        assert_eq!(migrations().table_name, LEDGER);
        assert_ne!(migrations().table_name, Migrator::DEFAULT.table_name);
    }
}
