use std::error::Error;

use async_trait::async_trait;
use email::EmailAddress;
use eventsourcing::{AccountId, Version};
use invitations_core::{
    CatalogError, InvitationCatalog, InvitationId, InvitationSummary, Person, ProjectLink,
};
use sqlx::migrate::Migrator;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

use crate::{outcome, scope};

const REMEMBER: &str = "
    INSERT INTO invitation_summaries
        (invitation, version, email, issued_at, until, outcome, declined_for, retire_after, project, project_role)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
    ON CONFLICT (invitation) DO UPDATE
    SET version      = EXCLUDED.version,
        email        = EXCLUDED.email,
        issued_at    = EXCLUDED.issued_at,
        until        = EXCLUDED.until,
        outcome      = EXCLUDED.outcome,
        declined_for = EXCLUDED.declined_for,
        retire_after = EXCLUDED.retire_after,
        project      = EXCLUDED.project,
        project_role = EXCLUDED.project_role
    WHERE invitation_summaries.version < EXCLUDED.version
";

const LISTED_AFTER: &str = "
    SELECT invitation, version, email, issued_at, until, outcome, declined_for, retire_after, project, project_role
    FROM invitation_summaries
    WHERE outcome IN ('open', 'declined')
      AND project IS NULL
      AND ($1::TEXT IS NULL OR invitation < $1)
    ORDER BY invitation DESC
    LIMIT $2
";

const IN_PROJECT_AFTER: &str = "
    SELECT invitation, version, email, issued_at, until, outcome, declined_for, retire_after, project, project_role
    FROM invitation_summaries
    WHERE outcome IN ('open', 'declined')
      AND project = $1
      AND ($2::TEXT IS NULL OR invitation < $2)
    ORDER BY invitation DESC
    LIMIT $3
";

const DUE_FOR_RETIRING: &str = "
    SELECT invitation
    FROM invitation_summaries
    WHERE retire_after < $1
    ORDER BY retire_after
    LIMIT $2
";

const FORGET: &str = "DELETE FROM invitation_summaries WHERE invitation = $1";

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

const LEDGER: &str = "_sqlx_migrations_invitation_summaries";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/postgres");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct PostgresInvitationCatalog {
    pool: PgPool,
}

impl PostgresInvitationCatalog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

fn to_summary(row: &PgRow) -> Result<InvitationSummary, CatalogError> {
    let invitation: String = row.try_get("invitation").map_err(unreachable)?;
    let version: i64 = row.try_get("version").map_err(unreachable)?;
    let email: String = row.try_get("email").map_err(unreachable)?;
    let issued_at: OffsetDateTime = row.try_get("issued_at").map_err(unreachable)?;
    let until: OffsetDateTime = row.try_get("until").map_err(unreachable)?;
    let kept: String = row.try_get("outcome").map_err(unreachable)?;
    let declined_for: Option<String> = row.try_get("declined_for").map_err(unreachable)?;
    let project: Option<String> = row.try_get("project").map_err(unreachable)?;
    let project_role: Option<String> = row.try_get("project_role").map_err(unreachable)?;
    let retire_after: OffsetDateTime = row.try_get("retire_after").map_err(unreachable)?;

    Ok(InvitationSummary {
        id: invitation.parse().map_err(unreachable)?,
        version: Version::of(u64::try_from(version).map_err(unreachable)?),
        to: EmailAddress::new(&email).map_err(unreachable)?,
        scope: scope::read(project, project_role.as_deref())?,
        issued_at,
        until,
        outcome: outcome::read(&kept, declined_for.as_deref())?,
        retire_after,
    })
}

#[async_trait]
impl InvitationCatalog for PostgresInvitationCatalog {
    async fn remember(&self, summary: &InvitationSummary) -> Result<(), CatalogError> {
        let (kept, declined_for) = outcome::written(summary.outcome);
        let (project, project_role) = scope::written(&summary.scope);

        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(i64::try_from(summary.version.count()).map_err(unreachable)?)
            .bind(summary.to.as_str())
            .bind(summary.issued_at)
            .bind(summary.until)
            .bind(kept)
            .bind(declined_for)
            .bind(summary.retire_after)
            .bind(project)
            .bind(project_role)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn listed_after(
        &self,
        after: Option<&InvitationId>,
        at_most: usize,
    ) -> Result<Vec<InvitationSummary>, CatalogError> {
        let found = sqlx::query(LISTED_AFTER)
            .bind(after.map(ToString::to_string))
            .bind(i64::try_from(at_most).map_err(unreachable)?)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found.iter().map(to_summary).collect()
    }

    async fn in_project_after(
        &self,
        project: &ProjectLink,
        after: Option<&InvitationId>,
        at_most: usize,
    ) -> Result<Vec<InvitationSummary>, CatalogError> {
        let found = sqlx::query(IN_PROJECT_AFTER)
            .bind(project.as_str())
            .bind(after.map(ToString::to_string))
            .bind(i64::try_from(at_most).map_err(unreachable)?)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found.iter().map(to_summary).collect()
    }

    async fn due_for_retiring(
        &self,
        now: OffsetDateTime,
        at_most: usize,
    ) -> Result<Vec<InvitationId>, CatalogError> {
        let due: Vec<String> = sqlx::query_scalar(DUE_FOR_RETIRING)
            .bind(now)
            .bind(i64::try_from(at_most).map_err(unreachable)?)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        due.iter()
            .map(|invitation| invitation.parse().map_err(unreachable))
            .collect()
    }

    async fn forget(&self, invitation: &InvitationId) -> Result<(), CatalogError> {
        sqlx::query(FORGET)
            .bind(invitation.to_string())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn remember_person(&self, person: &Person) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER_PERSON)
            .bind(person.account.as_str())
            .bind(&person.name)
            .bind(person.email.as_str())
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
    let email: String = row.try_get("email").map_err(unreachable)?;

    Ok(Person {
        account: AccountId::from(account),
        name: row.try_get("name").map_err(unreachable)?,
        email: EmailAddress::new(&email).map_err(unreachable)?,
    })
}

#[cfg(test)]
mod ledger_tests {
    use super::*;

    #[test]
    fn the_list_keeps_a_ledger_of_its_own_beside_the_events_one() {
        assert_eq!(migrations().table_name, LEDGER);
        assert_ne!(migrations().table_name, Migrator::DEFAULT.table_name);
    }
}
