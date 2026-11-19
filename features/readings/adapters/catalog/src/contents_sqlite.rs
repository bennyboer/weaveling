use async_trait::async_trait;
use readings_core::{CatalogError, Contents, ReadingCopyContents, ReadingCopyId};
use sqlx::{Row, SqlitePool};
use transacting::Transaction;
use transacting::sqlite::connection;

use crate::kept;
use crate::sqlite::unreachable;

const KEEP: &str = "
    INSERT INTO reading_copy_contents (copy, project, contents)
    VALUES (?1, ?2, ?3)
    ON CONFLICT (copy) DO NOTHING
";

const CONTENTS_OF: &str = "SELECT project, contents FROM reading_copy_contents WHERE copy = ?1";

const FORGET: &str = "DELETE FROM reading_copy_contents WHERE copy = ?1";

pub struct SqliteReadingCopyContents {
    pool: SqlitePool,
}

impl SqliteReadingCopyContents {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl ReadingCopyContents for SqliteReadingCopyContents {
    async fn keep(
        &self,
        transaction: &mut Transaction,
        copy: &ReadingCopyId,
        contents: &Contents,
    ) -> Result<(), CatalogError> {
        sqlx::query(KEEP)
            .bind(copy.to_string())
            .bind(contents.project.as_str())
            .bind(kept::contents::to_json(contents))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn contents_of(&self, copy: &ReadingCopyId) -> Result<Option<Contents>, CatalogError> {
        let Some(row) = sqlx::query(CONTENTS_OF)
            .bind(copy.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?
        else {
            return Ok(None);
        };
        let project: String = row.try_get("project").map_err(unreachable)?;
        let contents: String = row.try_get("contents").map_err(unreachable)?;

        Ok(Some(kept::contents::from_json(&project, &contents)?))
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
}
