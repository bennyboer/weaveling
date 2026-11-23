use std::collections::HashMap;

use async_trait::async_trait;
use feedback_core::{
    CatalogError, KnownReadingCopy, ProjectLink, ReadingCopyCatalog, ReadingCopyLink,
};
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use transacting::Transaction;
use transacting::postgres::connection;

use crate::kept::{from_microseconds, in_microseconds};
use crate::postgres::unreachable;

const KEEP: &str = "
    INSERT INTO reading_copies_heard (reading_copy, project, open_until, version)
    SELECT $1, $2, $3, $4
    WHERE NOT EXISTS (SELECT 1 FROM reading_copy_tombstones WHERE reading_copy = $1)
    ON CONFLICT (reading_copy) DO UPDATE
    SET project = excluded.project, open_until = excluded.open_until, version = excluded.version
    WHERE reading_copies_heard.version < excluded.version
";

const LAY_TOMBSTONE: &str = "INSERT INTO reading_copy_tombstones (reading_copy, laid_at) VALUES ($1, $2) ON CONFLICT DO NOTHING";

const FORGET: &str = "DELETE FROM reading_copies_heard WHERE reading_copy = $1";

const FORGET_OLD_TOMBSTONES: &str = "
    DELETE FROM reading_copy_tombstones WHERE reading_copy IN (
        SELECT reading_copy FROM reading_copy_tombstones
        WHERE laid_at < $1
        ORDER BY laid_at, reading_copy
        LIMIT $2
    )
";

const KNOWN: &str = "SELECT reading_copy, project, open_until FROM reading_copies_heard WHERE reading_copy = ANY($1)";

pub struct PostgresReadingCopyCatalog {
    pool: PgPool,
}

impl PostgresReadingCopyCatalog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ReadingCopyCatalog for PostgresReadingCopyCatalog {
    async fn keep(
        &self,
        transaction: &mut Transaction,
        copy: &ReadingCopyLink,
        known: &KnownReadingCopy,
        version: u64,
    ) -> Result<bool, CatalogError> {
        let kept = sqlx::query(KEEP)
            .bind(copy.as_str())
            .bind(known.project.as_str())
            .bind(known.open_until.map(in_microseconds))
            .bind(i64::try_from(version).map_err(unreachable)?)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(kept.rows_affected() > 0)
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        copy: &ReadingCopyLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        sqlx::query(LAY_TOMBSTONE)
            .bind(copy.as_str())
            .bind(in_microseconds(at))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;
        sqlx::query(FORGET)
            .bind(copy.as_str())
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        transaction: &mut Transaction,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        Ok(sqlx::query(FORGET_OLD_TOMBSTONES)
            .bind(in_microseconds(before))
            .bind(i64::try_from(at_most).unwrap_or(i64::MAX))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?
            .rows_affected())
    }

    async fn known(
        &self,
        copies: &[ReadingCopyLink],
    ) -> Result<HashMap<ReadingCopyLink, KnownReadingCopy>, CatalogError> {
        let asked: Vec<&str> = copies.iter().map(ReadingCopyLink::as_str).collect();

        sqlx::query(KNOWN)
            .bind(asked)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
            .iter()
            .map(|row| {
                let copy: String = row.try_get("reading_copy").map_err(unreachable)?;
                let project: String = row.try_get("project").map_err(unreachable)?;
                let open_until: Option<i64> = row.try_get("open_until").map_err(unreachable)?;

                Ok((
                    ReadingCopyLink::from(copy),
                    KnownReadingCopy {
                        project: ProjectLink::from(project),
                        open_until: open_until.map(from_microseconds).transpose()?,
                    },
                ))
            })
            .collect()
    }
}
