use std::error::Error;

use async_trait::async_trait;
use invitations_core::{InvitationId, InvitationLinks, InvitationTokenDigest, LinksError};
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;

const KEEP: &str = "
    INSERT INTO invitation_links (digest, invitation)
    VALUES (?1, ?2)
    ON CONFLICT (digest) DO UPDATE
    SET invitation = EXCLUDED.invitation
";

const INVITATION_OF: &str = "SELECT invitation FROM invitation_links WHERE digest = ?1";

const FORGET: &str = "DELETE FROM invitation_links WHERE invitation = ?1";

const LEDGER: &str = "_sqlx_migrations_invitation_links";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/sqlite");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct SqliteInvitationLinks {
    pool: SqlitePool,
}

impl SqliteInvitationLinks {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl Error + Send + Sync + 'static) -> LinksError {
    LinksError::Backend(Box::new(failure))
}

#[async_trait]
impl InvitationLinks for SqliteInvitationLinks {
    async fn keep(
        &self,
        digest: &InvitationTokenDigest,
        invitation: &InvitationId,
    ) -> Result<(), LinksError> {
        sqlx::query(KEEP)
            .bind(digest.as_str())
            .bind(invitation.to_string())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn invitation_of(
        &self,
        digest: &InvitationTokenDigest,
    ) -> Result<Option<InvitationId>, LinksError> {
        let found: Option<String> = sqlx::query_scalar(INVITATION_OF)
            .bind(digest.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?;

        found
            .map(|invitation| invitation.parse().map_err(unreachable))
            .transpose()
    }

    async fn forget(&self, invitation: &InvitationId) -> Result<(), LinksError> {
        sqlx::query(FORGET)
            .bind(invitation.to_string())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }
}

#[cfg(test)]
mod ledger_tests {
    use super::*;

    #[test]
    fn the_links_keep_a_ledger_of_their_own() {
        assert_eq!(migrations().table_name, LEDGER);
        assert_ne!(
            migrations().table_name,
            Migrator::DEFAULT.table_name,
            "the invitations' catalog will lay down a 0001 of its own beside them"
        );
    }
}
