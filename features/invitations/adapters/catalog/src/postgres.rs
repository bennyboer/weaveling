use std::error::Error;

use async_trait::async_trait;
use email::EmailAddress;
use eventsourcing::Version;
use invitations_core::{CatalogError, InvitationCatalog, InvitationId, InvitationSummary};
use sqlx::migrate::Migrator;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

use crate::outcome;

const REMEMBER: &str = "
    INSERT INTO invitation_summaries
        (invitation, version, email, issued_at, until, outcome, declined_for)
    VALUES ($1, $2, $3, $4, $5, $6, $7)
    ON CONFLICT (invitation) DO UPDATE
    SET version      = EXCLUDED.version,
        email        = EXCLUDED.email,
        issued_at    = EXCLUDED.issued_at,
        until        = EXCLUDED.until,
        outcome      = EXCLUDED.outcome,
        declined_for = EXCLUDED.declined_for
    WHERE invitation_summaries.version < EXCLUDED.version
";

const LISTED_AFTER: &str = "
    SELECT invitation, version, email, issued_at, until, outcome, declined_for
    FROM invitation_summaries
    WHERE outcome IN ('open', 'declined')
      AND ($1::TEXT IS NULL OR invitation < $1)
    ORDER BY invitation DESC
    LIMIT $2
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

    Ok(InvitationSummary {
        id: invitation.parse().map_err(unreachable)?,
        version: Version::of(u64::try_from(version).map_err(unreachable)?),
        to: EmailAddress::new(&email).map_err(unreachable)?,
        issued_at,
        until,
        outcome: outcome::read(&kept, declined_for.as_deref())?,
    })
}

#[async_trait]
impl InvitationCatalog for PostgresInvitationCatalog {
    async fn remember(&self, summary: &InvitationSummary) -> Result<(), CatalogError> {
        let (kept, declined_for) = outcome::written(summary.outcome);

        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(i64::try_from(summary.version.count()).map_err(unreachable)?)
            .bind(summary.to.as_str())
            .bind(summary.issued_at)
            .bind(summary.until)
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

#[cfg(test)]
mod ledger_tests {
    use super::*;

    #[test]
    fn the_list_keeps_a_ledger_of_its_own_beside_the_events_one() {
        assert_eq!(migrations().table_name, LEDGER);
        assert_ne!(migrations().table_name, Migrator::DEFAULT.table_name);
    }
}
