use std::error::Error;

use async_trait::async_trait;
use eventsourcing::AccountId;
use members_core::{
    CatalogError, Membership, MembershipCatalog, Person, ProjectLink, ProjectMembers, Role,
};
use sqlx::migrate::Migrator;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};

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
        let asked: Vec<&str> = accounts.iter().map(AccountId::as_str).collect();
        let found = sqlx::query(PEOPLE)
            .bind(asked)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found.iter().map(person_in).collect()
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
        assert_eq!(migrations().table_name, LEDGER);
        assert_ne!(migrations().table_name, Migrator::DEFAULT.table_name);
    }
}
