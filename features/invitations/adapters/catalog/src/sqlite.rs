use std::error::Error;

use async_trait::async_trait;
use clock::text;
use email::EmailAddress;
use eventsourcing::{AccountId, Version};
use invitations_core::{
    CatalogError, InvitationCatalog, InvitationId, InvitationSummary, Person, ProjectLink,
};
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use time::OffsetDateTime;

use crate::{outcome, scope};

const REMEMBER: &str = "
    INSERT INTO invitation_summaries
        (invitation, version, email, issued_at, until, outcome, declined_for, retire_after, project, project_role)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
    ON CONFLICT (invitation) DO UPDATE
    SET version      = excluded.version,
        email        = excluded.email,
        issued_at    = excluded.issued_at,
        until        = excluded.until,
        outcome      = excluded.outcome,
        declined_for = excluded.declined_for,
        retire_after = excluded.retire_after,
        project      = excluded.project,
        project_role = excluded.project_role
    WHERE invitation_summaries.version < excluded.version
";

const LISTED_AFTER: &str = "
    SELECT invitation, version, email, issued_at, until, outcome, declined_for, retire_after, project, project_role
    FROM invitation_summaries
    WHERE outcome IN ('open', 'declined')
      AND project IS NULL
      AND (?1 IS NULL OR invitation < ?1)
    ORDER BY invitation DESC
    LIMIT ?2
";

const IN_PROJECT_AFTER: &str = "
    SELECT invitation, version, email, issued_at, until, outcome, declined_for, retire_after, project, project_role
    FROM invitation_summaries
    WHERE outcome IN ('open', 'declined')
      AND project = ?1
      AND (?2 IS NULL OR invitation < ?2)
    ORDER BY invitation DESC
    LIMIT ?3
";

const DUE_FOR_RETIRING: &str = "
    SELECT invitation
    FROM invitation_summaries
    WHERE retire_after < ?1
    ORDER BY retire_after
    LIMIT ?2
";

const FORGET: &str = "DELETE FROM invitation_summaries WHERE invitation = ?1";

const REMEMBER_PERSON: &str = "
    INSERT INTO people (account, name, email)
    VALUES (?1, ?2, ?3)
    ON CONFLICT (account) DO UPDATE
    SET name = excluded.name, email = excluded.email
";

const PERSON: &str = "SELECT account, name, email FROM people WHERE account = ?1";

const LEDGER: &str = "_sqlx_migrations_invitation_summaries";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/sqlite");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct SqliteInvitationCatalog {
    pool: SqlitePool,
}

impl SqliteInvitationCatalog {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

fn moment(row: &SqliteRow, column: &str) -> Result<OffsetDateTime, CatalogError> {
    let written: String = row.try_get(column).map_err(unreachable)?;

    text::read(&written).ok_or_else(|| {
        CatalogError::Backend(format!("{column} holds {written:?}, which is no time").into())
    })
}

fn to_summary(row: &SqliteRow) -> Result<InvitationSummary, CatalogError> {
    let invitation: String = row.try_get("invitation").map_err(unreachable)?;
    let version: i64 = row.try_get("version").map_err(unreachable)?;
    let email: String = row.try_get("email").map_err(unreachable)?;
    let kept: String = row.try_get("outcome").map_err(unreachable)?;
    let declined_for: Option<String> = row.try_get("declined_for").map_err(unreachable)?;
    let project: Option<String> = row.try_get("project").map_err(unreachable)?;
    let project_role: Option<String> = row.try_get("project_role").map_err(unreachable)?;

    Ok(InvitationSummary {
        id: invitation.parse().map_err(unreachable)?,
        version: Version::of(u64::try_from(version).map_err(unreachable)?),
        to: EmailAddress::new(&email).map_err(unreachable)?,
        scope: scope::read(project, project_role.as_deref())?,
        issued_at: moment(row, "issued_at")?,
        until: moment(row, "until")?,
        outcome: outcome::read(&kept, declined_for.as_deref())?,
        retire_after: moment(row, "retire_after")?,
    })
}

#[async_trait]
impl InvitationCatalog for SqliteInvitationCatalog {
    async fn remember(&self, summary: &InvitationSummary) -> Result<(), CatalogError> {
        let (kept, declined_for) = outcome::written(summary.outcome);
        let (project, project_role) = scope::written(&summary.scope);

        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(i64::try_from(summary.version.count()).map_err(unreachable)?)
            .bind(summary.to.as_str())
            .bind(text::written(summary.issued_at))
            .bind(text::written(summary.until))
            .bind(kept)
            .bind(declined_for)
            .bind(text::written(summary.retire_after))
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
            .bind(text::written(now))
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
    let email: String = row.try_get("email").map_err(unreachable)?;

    Ok(Person {
        account: AccountId::from(account),
        name: row.try_get("name").map_err(unreachable)?,
        email: EmailAddress::new(&email).map_err(unreachable)?,
    })
}
