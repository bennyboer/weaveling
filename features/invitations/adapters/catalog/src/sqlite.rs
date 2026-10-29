use std::error::Error;

use async_trait::async_trait;
use clock::text;
use email::EmailAddress;
use eventsourcing::{AccountId, Version};
use invitations_core::{
    CatalogError, InvitationCatalog, InvitationId, InvitationSummary, InvitationTokenDigest,
    Person, ProjectLink, ProjectName,
};
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use time::OffsetDateTime;

use crate::{outcome, scope};

const REMEMBER: &str = "
    INSERT INTO invitation_summaries
        (invitation, version, email, issued_at, until, outcome, declined_for, retire_after, project, project_role, issued_by, digest)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12)
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
    SELECT invitation, version, email, issued_at, until, outcome, declined_for, retire_after, project, project_role, issued_by, digest
    FROM invitation_summaries
    WHERE outcome IN ('open', 'declined')
      AND project IS NULL
      AND (?1 IS NULL OR invitation < ?1)
    ORDER BY invitation DESC
    LIMIT ?2
";

const IN_PROJECT_AFTER: &str = "
    SELECT invitation, version, email, issued_at, until, outcome, declined_for, retire_after, project, project_role, issued_by, digest
    FROM invitation_summaries
    WHERE outcome IN ('open', 'declined')
      AND project = ?1
      AND (?2 IS NULL OR invitation < ?2)
    ORDER BY invitation DESC
    LIMIT ?3
";

const OPEN_TO: &str = "
    SELECT invitation, version, email, issued_at, until, outcome, declined_for, retire_after, project, project_role, issued_by, digest
    FROM invitation_summaries
    WHERE email = ?1 AND outcome = 'open'
    ORDER BY invitation DESC
";

const WITH_DIGEST: &str = "
    SELECT invitation, version, email, issued_at, until, outcome, declined_for, retire_after, project, project_role, issued_by, digest
    FROM invitation_summaries
    WHERE digest = ?1
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

const REMEMBER_PROJECT_NAME: &str = "
    INSERT INTO project_names (project, version, name)
    SELECT ?1, ?2, ?3
    WHERE NOT EXISTS (SELECT 1 FROM project_tombstones WHERE project = ?1)
    ON CONFLICT (project) DO UPDATE
    SET version = excluded.version, name = excluded.name
    WHERE project_names.version < excluded.version
";

const PROJECT_NAME: &str = "SELECT project, version, name FROM project_names WHERE project = ?1";

const LAY_PROJECT_TOMBSTONE: &str =
    "INSERT INTO project_tombstones (project, laid_at) VALUES (?1, ?2) ON CONFLICT DO NOTHING";

const FORGET_PROJECT_NAME: &str = "DELETE FROM project_names WHERE project = ?1";

const HAS_PROJECT_TOMBSTONE: &str =
    "SELECT EXISTS (SELECT 1 FROM project_tombstones WHERE project = ?1)";

const FORGET_OLD_PROJECT_TOMBSTONES: &str = "
    DELETE FROM project_tombstones WHERE project IN (
        SELECT project FROM project_tombstones
        WHERE laid_at < ?1
        ORDER BY laid_at, project
        LIMIT ?2
    )
";

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
    let issued_by: Option<String> = row.try_get("issued_by").map_err(unreachable)?;
    let digest: String = row.try_get("digest").map_err(unreachable)?;

    Ok(InvitationSummary {
        id: invitation.parse().map_err(unreachable)?,
        version: Version::of(u64::try_from(version).map_err(unreachable)?),
        to: EmailAddress::new(&email).map_err(unreachable)?,
        digest: InvitationTokenDigest::kept(digest),
        scope: scope::read(project, project_role.as_deref())?,
        issued_by: issued_by.map(AccountId::from),
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
            .bind(summary.issued_by.as_ref().map(AccountId::as_str))
            .bind(summary.digest.as_str())
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

    async fn open_to(&self, to: &EmailAddress) -> Result<Vec<InvitationSummary>, CatalogError> {
        let found = sqlx::query(OPEN_TO)
            .bind(to.as_str())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found.iter().map(to_summary).collect()
    }

    async fn with_digest(
        &self,
        digest: &InvitationTokenDigest,
    ) -> Result<Option<InvitationSummary>, CatalogError> {
        sqlx::query(WITH_DIGEST)
            .bind(digest.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?
            .as_ref()
            .map(to_summary)
            .transpose()
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

    async fn remember_project_name(&self, named: &ProjectName) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER_PROJECT_NAME)
            .bind(named.project.as_str())
            .bind(i64::try_from(named.version.count()).map_err(unreachable)?)
            .bind(&named.name)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn lay_project_tombstone(
        &self,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        let mut transaction = self.pool.begin().await.map_err(unreachable)?;
        sqlx::query(LAY_PROJECT_TOMBSTONE)
            .bind(project.as_str())
            .bind(text::written(at))
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;
        sqlx::query(FORGET_PROJECT_NAME)
            .bind(project.as_str())
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        transaction.commit().await.map_err(unreachable)
    }

    async fn has_project_tombstone(&self, project: &ProjectLink) -> Result<bool, CatalogError> {
        sqlx::query_scalar(HAS_PROJECT_TOMBSTONE)
            .bind(project.as_str())
            .fetch_one(&self.pool)
            .await
            .map_err(unreachable)
    }

    async fn forget_project_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        Ok(sqlx::query(FORGET_OLD_PROJECT_TOMBSTONES)
            .bind(text::written(before))
            .bind(i64::try_from(at_most).unwrap_or(i64::MAX))
            .execute(&self.pool)
            .await
            .map_err(unreachable)?
            .rows_affected())
    }

    async fn project_names(
        &self,
        projects: &[ProjectLink],
    ) -> Result<Vec<ProjectName>, CatalogError> {
        let mut found = Vec::new();
        for project in projects {
            let row = sqlx::query(PROJECT_NAME)
                .bind(project.as_str())
                .fetch_optional(&self.pool)
                .await
                .map_err(unreachable)?;
            if let Some(row) = row {
                found.push(project_name_in(&row)?);
            }
        }
        found.sort_by(|one, other| one.project.as_str().cmp(other.project.as_str()));

        Ok(found)
    }
}

fn project_name_in(row: &SqliteRow) -> Result<ProjectName, CatalogError> {
    let project: String = row.try_get("project").map_err(unreachable)?;
    let version: i64 = row.try_get("version").map_err(unreachable)?;

    Ok(ProjectName {
        project: ProjectLink::from(project),
        version: Version::of(u64::try_from(version).map_err(unreachable)?),
        name: row.try_get("name").map_err(unreachable)?,
    })
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
