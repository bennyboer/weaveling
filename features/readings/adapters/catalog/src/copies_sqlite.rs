use async_trait::async_trait;
use readings_core::{
    CatalogError, ProjectLink, ReadingCopyCatalog, ReadingCopyId, ReadingCopySummary,
};
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use time::OffsetDateTime;
use transacting::Transaction;
use transacting::sqlite::connection;

use crate::copies_kept::{self, KeptReadingCopySummary};
use crate::sqlite::unreachable;

const REMEMBER: &str = "
    INSERT INTO reading_copies
        (copy, project, name, made_by_kind, made_by, made_at, extent, extent_count, open_until, deleted_on)
    VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
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

const FORGET: &str = "DELETE FROM reading_copies WHERE copy = ?1";

const COPIES_OF: &str = "
    SELECT copy, project, name, made_by_kind, made_by, made_at, extent, extent_count, open_until, deleted_on
    FROM reading_copies
    WHERE project = ?1
    ORDER BY made_at DESC, copy DESC
";

pub struct SqliteReadingCopyCatalog {
    pool: SqlitePool,
}

impl SqliteReadingCopyCatalog {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn moment_in(row: &SqliteRow, column: &str) -> Result<Option<OffsetDateTime>, CatalogError> {
    row.try_get::<Option<String>, _>(column)
        .map_err(unreachable)?
        .map(|written| {
            clock::text::read(&written)
                .ok_or_else(|| CatalogError::Backend(format!("{written} is not a moment").into()))
        })
        .transpose()
}

fn kept_in(row: &SqliteRow) -> Result<KeptReadingCopySummary, CatalogError> {
    Ok(KeptReadingCopySummary {
        copy: row.try_get("copy").map_err(unreachable)?,
        project: row.try_get("project").map_err(unreachable)?,
        name: row.try_get("name").map_err(unreachable)?,
        made_by_kind: row.try_get("made_by_kind").map_err(unreachable)?,
        made_by: row.try_get("made_by").map_err(unreachable)?,
        made_at: moment_in(row, "made_at")?
            .ok_or_else(|| CatalogError::Backend("a copy was made at no moment".into()))?,
        extent: row.try_get("extent").map_err(unreachable)?,
        extent_count: row.try_get("extent_count").map_err(unreachable)?,
        open_until: moment_in(row, "open_until")?,
        deleted_on: moment_in(row, "deleted_on")?,
    })
}

#[async_trait]
impl ReadingCopyCatalog for SqliteReadingCopyCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        summary: &ReadingCopySummary,
    ) -> Result<(), CatalogError> {
        let (made_by_kind, made_by) = copies_kept::maker_written(&summary.made_by);
        let (extent, extent_count) = copies_kept::extent_written(summary.extent);
        sqlx::query(REMEMBER)
            .bind(summary.copy.to_string())
            .bind(summary.project.as_str())
            .bind(summary.name.to_string())
            .bind(made_by_kind)
            .bind(made_by)
            .bind(clock::text::written(summary.made_at))
            .bind(extent)
            .bind(extent_count)
            .bind(summary.times.open_until.map(clock::text::written))
            .bind(summary.times.deleted_on.map(clock::text::written))
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
            .map(|row| kept_in(row)?.summary())
            .collect()
    }
}
