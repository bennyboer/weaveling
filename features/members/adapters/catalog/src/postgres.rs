use std::error::Error;

use async_trait::async_trait;
use eventsourcing::AccountId;
use members_core::{
    CatalogError, Membership, MembershipCatalog, ProjectLink, ProjectMembers, Role,
};
use sqlx::migrate::Migrator;
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
