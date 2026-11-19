use async_trait::async_trait;
use readings_core::{
    CatalogError, ProjectLink, ReadingCopyCatalog, ReadingCopyId, ReadingCopySummary,
};
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use transacting::Transaction;
use transacting::postgres::connection;

use crate::kept;
use crate::kept::copies::KeptReadingCopySummary;
use crate::postgres::unreachable;

const REMEMBER: &str = "
    INSERT INTO reading_copies
        (copy, project, name, made_by_kind, made_by, made_at, extent, extent_count, open_until, deleted_on)
    VALUES ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10)
    ON CONFLICT (copy) DO UPDATE SET
        project = excluded.project,
        name = excluded.name,
        made_by_kind = excluded.made_by_kind,
        made_by = excluded.made_by,
        made_at = excluded.made_at,
        extent = excluded.extent,
        extent_count = excluded.extent_count,
        open_until = excluded.open_until,
        deleted_on = excluded.deleted_on
";

const FORGET: &str = "DELETE FROM reading_copies WHERE copy = $1";

const COPIES_OF: &str = "
    SELECT copy, project, name, made_by_kind, made_by, made_at, extent, extent_count, open_until, deleted_on
    FROM reading_copies
    WHERE project = $1
    ORDER BY made_at DESC, copy DESC
";

const DUE_FOR_DELETION: &str = "
    SELECT copy FROM reading_copies
    WHERE deleted_on <= $1
    ORDER BY deleted_on, copy
    LIMIT $2
";

pub struct PostgresReadingCopyCatalog {
    pool: PgPool,
}

impl PostgresReadingCopyCatalog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn kept_in(row: &PgRow) -> Result<KeptReadingCopySummary, sqlx::Error> {
    Ok(KeptReadingCopySummary {
        copy: row.try_get("copy")?,
        project: row.try_get("project")?,
        name: row.try_get("name")?,
        made_by_kind: row.try_get("made_by_kind")?,
        made_by: row.try_get("made_by")?,
        made_at: row.try_get("made_at")?,
        extent: row.try_get("extent")?,
        extent_count: row.try_get("extent_count")?,
        open_until: row.try_get("open_until")?,
        deleted_on: row.try_get("deleted_on")?,
    })
}

#[async_trait]
impl ReadingCopyCatalog for PostgresReadingCopyCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &ReadingCopySummary,
    ) -> Result<(), CatalogError> {
        let (made_by_kind, made_by) = kept::copies::maker_written(&summary.made_by);
        let (extent, extent_count) = kept::copies::extent_written(summary.extent);
        sqlx::query(REMEMBER)
            .bind(summary.copy.to_string())
            .bind(summary.project.as_str())
            .bind(summary.name.to_string())
            .bind(made_by_kind)
            .bind(made_by)
            .bind(summary.made_at)
            .bind(extent)
            .bind(extent_count)
            .bind(summary.times.open_until)
            .bind(summary.times.deleted_on)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        copy: &ReadingCopyId,
    ) -> Result<(), CatalogError> {
        sqlx::query(FORGET)
            .bind(copy.to_string())
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn copies_of(
        &self,
        project: &ProjectLink,
    ) -> Result<Vec<ReadingCopySummary>, CatalogError> {
        sqlx::query(COPIES_OF)
            .bind(project.as_str())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
            .iter()
            .map(|row| kept_in(row).map_err(unreachable)?.summary())
            .collect()
    }

    async fn due_for_deletion(
        &self,
        at: OffsetDateTime,
        at_most: u64,
    ) -> Result<Vec<ReadingCopyId>, CatalogError> {
        sqlx::query(DUE_FOR_DELETION)
            .bind(at)
            .bind(i64::try_from(at_most).unwrap_or(i64::MAX))
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
            .iter()
            .map(|row| {
                let copy: String = row.try_get("copy").map_err(unreachable)?;

                copy.parse::<ReadingCopyId>()
                    .map_err(|why| CatalogError::Backend(why.to_string().into()))
            })
            .collect()
    }
}
