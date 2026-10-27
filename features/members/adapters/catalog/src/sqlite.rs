use std::error::Error;

use async_trait::async_trait;
use eventsourcing::AccountId;
use members_core::{
    CatalogError, Membership, MembershipCatalog, Person, ProjectLink, ProjectMembers, Role,
};
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

use crate::role;

const CLAIM: &str = "
    INSERT INTO project_member_lists (project, version)
    VALUES (?1, ?2)
    ON CONFLICT (project) DO UPDATE
    SET version = excluded.version
    WHERE project_member_lists.version < excluded.version
";

const CLEAR: &str = "DELETE FROM memberships WHERE project = ?1";

const ADD: &str = "INSERT INTO memberships (project, account, role) VALUES (?1, ?2, ?3)";

const ROLE_OF: &str = "SELECT role FROM memberships WHERE project = ?1 AND account = ?2";

const MEMBERSHIPS_OF: &str = "
    SELECT project, role
    FROM memberships
    WHERE account = ?1
    ORDER BY project
";

const REMEMBER_PERSON: &str = "
    INSERT INTO people (account, name, email)
    VALUES (?1, ?2, ?3)
    ON CONFLICT (account) DO UPDATE
    SET name = excluded.name, email = excluded.email
";

const PERSON: &str = "SELECT account, name, email FROM people WHERE account = ?1";

const LEDGER: &str = "_sqlx_migrations_memberships";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/sqlite");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct SqliteMembershipCatalog {
    pool: SqlitePool,
}

impl SqliteMembershipCatalog {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl MembershipCatalog for SqliteMembershipCatalog {
    async fn remember(&self, members: &ProjectMembers) -> Result<(), CatalogError> {
        let mut transaction = self.pool.begin().await.map_err(unreachable)?;
        let claimed = sqlx::query(CLAIM)
            .bind(members.project.as_str())
            .bind(i64::try_from(members.version.count()).map_err(unreachable)?)
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        if claimed.rows_affected() > 0 {
            sqlx::query(CLEAR)
                .bind(members.project.as_str())
                .execute(&mut *transaction)
                .await
                .map_err(unreachable)?;
            for member in &members.members {
                sqlx::query(ADD)
                    .bind(members.project.as_str())
                    .bind(member.account.as_str())
                    .bind(role::written(member.role))
                    .execute(&mut *transaction)
                    .await
                    .map_err(unreachable)?;
            }
        }

        transaction.commit().await.map_err(unreachable)
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

    async fn remember_person(&self, person: &Person) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER_PERSON)
            .bind(person.account.as_str())
            .bind(&person.name)
            .bind(&person.email)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn people(&self, accounts: &[AccountId]) -> Result<Vec<Person>, CatalogError> {
        let mut found = Vec::new();
        for account in accounts {
            let row = sqlx::query(PERSON)
                .bind(account.as_str())
                .fetch_optional(&self.pool)
                .await
                .map_err(unreachable)?;
            if let Some(row) = row {
                found.push(person_in(&row)?);
            }
        }
        found.sort_by(|one, other| one.account.as_str().cmp(other.account.as_str()));

        Ok(found)
    }
}

fn person_in(row: &SqliteRow) -> Result<Person, CatalogError> {
    let account: String = row.try_get("account").map_err(unreachable)?;

    Ok(Person {
        account: AccountId::from(account),
        name: row.try_get("name").map_err(unreachable)?,
        email: row.try_get("email").map_err(unreachable)?,
    })
}
