use std::error::Error;

use async_trait::async_trait;
use clock::text;
use email::EmailAddress;
use eventsourcing::Version;
use invitations_core::{CatalogError, InvitationCatalog, InvitationId, InvitationSummary};
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use time::OffsetDateTime;

use crate::outcome;

const REMEMBER: &str = "
    INSERT INTO invitation_summaries
        (invitation, version, email, issued_at, until, outcome, declined_for)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)
    ON CONFLICT (invitation) DO UPDATE
    SET version      = excluded.version,
        email        = excluded.email,
        issued_at    = excluded.issued_at,
        until        = excluded.until,
        outcome      = excluded.outcome,
        declined_for = excluded.declined_for
    WHERE invitation_summaries.version < excluded.version
";

const LISTED_AFTER: &str = "
    SELECT invitation, version, email, issued_at, until, outcome, declined_for
    FROM invitation_summaries
    WHERE outcome IN ('open', 'declined')
      AND (?1 IS NULL OR invitation < ?1)
    ORDER BY invitation DESC
    LIMIT ?2
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

    Ok(InvitationSummary {
        id: invitation.parse().map_err(unreachable)?,
        version: Version::of(u64::try_from(version).map_err(unreachable)?),
        to: EmailAddress::new(&email).map_err(unreachable)?,
        issued_at: moment(row, "issued_at")?,
        until: moment(row, "until")?,
        outcome: outcome::read(&kept, declined_for.as_deref())?,
    })
}

#[async_trait]
impl InvitationCatalog for SqliteInvitationCatalog {
    async fn remember(&self, summary: &InvitationSummary) -> Result<(), CatalogError> {
        let (kept, declined_for) = outcome::written(summary.outcome);

        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(i64::try_from(summary.version.count()).map_err(unreachable)?)
            .bind(summary.to.as_str())
            .bind(text::written(summary.issued_at))
            .bind(text::written(summary.until))
            .bind(kept)
            .bind(declined_for)
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
}
