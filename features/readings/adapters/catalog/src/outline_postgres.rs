use async_trait::async_trait;
use readings_core::{BookOutline, CatalogError, OutlineCatalog, OutlinedSection, ProjectLink};
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use transacting::Transaction;
use transacting::postgres::connection;

use crate::kept;
use crate::postgres::{in_microseconds, unreachable};

const KEEP_OUTLINE: &str = "
    INSERT INTO books (project, sections, outline_version)
    SELECT $1, $2, $3
    WHERE NOT EXISTS (SELECT 1 FROM book_tombstones WHERE project = $1)
    ON CONFLICT (project) DO UPDATE
    SET sections = excluded.sections, outline_version = excluded.outline_version
    WHERE books.outline_version IS NULL OR books.outline_version < excluded.outline_version
";

const KEEP_TITLE: &str = "
    INSERT INTO books (project, title, title_version)
    SELECT $1, $2, $3
    WHERE NOT EXISTS (SELECT 1 FROM book_tombstones WHERE project = $1)
    ON CONFLICT (project) DO UPDATE
    SET title = excluded.title, title_version = excluded.title_version
    WHERE books.title_version IS NULL OR books.title_version < excluded.title_version
";

const LAY_TOMBSTONE: &str =
    "INSERT INTO book_tombstones (project, laid_at) VALUES ($1, $2) ON CONFLICT DO NOTHING";

const FORGET: &str = "DELETE FROM books WHERE project = $1";

const FORGET_OLD_TOMBSTONES: &str = "
    DELETE FROM book_tombstones WHERE project IN (
        SELECT project FROM book_tombstones
        WHERE laid_at < $1
        ORDER BY laid_at, project
        LIMIT $2
    )
";

const OUTLINE_OF: &str = "SELECT title, sections FROM books WHERE project = $1";

pub struct PostgresOutlineCatalog {
    pool: PgPool,
}

impl PostgresOutlineCatalog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl OutlineCatalog for PostgresOutlineCatalog {
    async fn keep_outline(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        sections: &[OutlinedSection],
        version: u64,
    ) -> Result<bool, CatalogError> {
        let kept = sqlx::query(KEEP_OUTLINE)
            .bind(project.as_str())
            .bind(kept::sections_to_json(sections))
            .bind(i64::try_from(version).map_err(unreachable)?)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(kept.rows_affected() > 0)
    }

    async fn keep_title(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        title: &str,
        version: u64,
    ) -> Result<bool, CatalogError> {
        let kept = sqlx::query(KEEP_TITLE)
            .bind(project.as_str())
            .bind(title)
            .bind(i64::try_from(version).map_err(unreachable)?)
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(kept.rows_affected() > 0)
    }

    async fn forget_book(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        sqlx::query(LAY_TOMBSTONE)
            .bind(project.as_str())
            .bind(in_microseconds(at))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;
        sqlx::query(FORGET)
            .bind(project.as_str())
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

    async fn outline_of(&self, project: &ProjectLink) -> Result<Option<BookOutline>, CatalogError> {
        let Some(row) = sqlx::query(OUTLINE_OF)
            .bind(project.as_str())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?
        else {
            return Ok(None);
        };
        let title: Option<String> = row.try_get("title").map_err(unreachable)?;
        let sections: Option<String> = row.try_get("sections").map_err(unreachable)?;

        Ok(Some(BookOutline {
            title: title.unwrap_or_default(),
            sections: sections
                .as_deref()
                .map(kept::sections_from_json)
                .transpose()?
                .unwrap_or_default(),
        }))
    }
}
