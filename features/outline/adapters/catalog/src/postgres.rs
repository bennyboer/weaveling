use async_trait::async_trait;
use outline_core::{
    Attachment, CatalogError, OutlineCatalog, OutlineId, OutlineSummary, ProjectLink,
};
use sqlx::migrate::Migrator;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

use crate::rows;

const REMEMBER: &str = "
    INSERT INTO outline_summaries (outline, project)
    VALUES ($1, $2)
    ON CONFLICT (outline) DO UPDATE SET project = EXCLUDED.project
";

const FORGET: &str = "DELETE FROM outline_summaries WHERE outline = $1";

const IN_PROJECT: &str = "
    SELECT outline, project
    FROM outline_summaries
    WHERE project = $1
    ORDER BY outline
";

const LET_GO: &str = "DELETE FROM outline_attachments WHERE outline = $1";

const HOLD: &str = "
    INSERT INTO outline_attachments (outline, attachment_type, attachment_id)
    SELECT $1, attachment_type, attachment_id
    FROM unnest($2::text[], $3::text[]) AS held (attachment_type, attachment_id)
    ON CONFLICT DO NOTHING
";

const HOLDING: &str = "
    SELECT outline
    FROM outline_attachments
    WHERE attachment_type = $1 AND attachment_id = $2
    ORDER BY outline
";

const LAY_TOMBSTONE: &str = "
    INSERT INTO attachment_tombstones (attachment_type, attachment_id, laid_at)
    VALUES ($1, $2, $3)
    ON CONFLICT DO NOTHING
";

const FORGET_OLD_TOMBSTONES: &str = "
    DELETE FROM attachment_tombstones WHERE (attachment_type, attachment_id) IN (
        SELECT attachment_type, attachment_id FROM attachment_tombstones
        WHERE laid_at < $1
        ORDER BY laid_at, attachment_type, attachment_id
        LIMIT $2
    )
";

const HELD_THOUGH_TOMBSTONED: &str = "
    SELECT held.attachment_type, held.attachment_id
    FROM outline_attachments held
    JOIN attachment_tombstones tombstone
        ON tombstone.attachment_type = held.attachment_type
        AND tombstone.attachment_id = held.attachment_id
    WHERE held.outline = $1
    ORDER BY held.attachment_id, held.attachment_type
";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations/postgres")
}

pub struct PostgresOutlineCatalog {
    pool: PgPool,
}

impl PostgresOutlineCatalog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl std::error::Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl OutlineCatalog for PostgresOutlineCatalog {
    async fn remember(&self, summary: &OutlineSummary) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(summary.project.to_string())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget(&self, outline: &OutlineId) -> Result<(), CatalogError> {
        let mut transaction = self.pool.begin().await.map_err(unreachable)?;

        sqlx::query(FORGET)
            .bind(outline.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        sqlx::query(LET_GO)
            .bind(outline.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        transaction.commit().await.map_err(unreachable)
    }

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<OutlineSummary>, CatalogError> {
        let found = sqlx::query(IN_PROJECT)
            .bind(project.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found
            .iter()
            .map(|row| {
                let outline: String = row.try_get("outline").map_err(unreachable)?;
                let project: String = row.try_get("project").map_err(unreachable)?;

                Ok(OutlineSummary {
                    id: outline.parse().map_err(unreachable)?,
                    project: ProjectLink::from(project.as_str()),
                })
            })
            .collect()
    }

    async fn holds(
        &self,
        outline: OutlineId,
        attachments: &[Attachment],
    ) -> Result<(), CatalogError> {
        let attachment_types: Vec<&str> =
            attachments.iter().map(rows::attachment_type_of).collect();
        let attachment_ids: Vec<String> = attachments.iter().map(ToString::to_string).collect();
        let mut transaction = self.pool.begin().await.map_err(unreachable)?;

        sqlx::query(LET_GO)
            .bind(outline.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        sqlx::query(HOLD)
            .bind(outline.to_string())
            .bind(&attachment_types)
            .bind(&attachment_ids)
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        transaction.commit().await.map_err(unreachable)
    }

    async fn outlines_holding(
        &self,
        attachment: &Attachment,
    ) -> Result<Vec<OutlineId>, CatalogError> {
        let found = sqlx::query(HOLDING)
            .bind(rows::attachment_type_of(attachment))
            .bind(attachment.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found
            .iter()
            .map(|row| {
                let outline: String = row.try_get("outline").map_err(unreachable)?;

                outline.parse().map_err(unreachable)
            })
            .collect()
    }

    async fn lay_tombstone(
        &self,
        attachment: &Attachment,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        sqlx::query(LAY_TOMBSTONE)
            .bind(rows::attachment_type_of(attachment))
            .bind(attachment.to_string())
            .bind(at)
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        Ok(sqlx::query(FORGET_OLD_TOMBSTONES)
            .bind(before)
            .bind(i64::try_from(at_most).unwrap_or(i64::MAX))
            .execute(&self.pool)
            .await
            .map_err(unreachable)?
            .rows_affected())
    }

    async fn held_though_tombstoned(
        &self,
        outline: OutlineId,
    ) -> Result<Vec<Attachment>, CatalogError> {
        let found = sqlx::query(HELD_THOUGH_TOMBSTONED)
            .bind(outline.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found
            .iter()
            .map(|row| {
                let attachment_type: String =
                    row.try_get("attachment_type").map_err(unreachable)?;
                let attachment_id: String = row.try_get("attachment_id").map_err(unreachable)?;

                rows::attachment_from(&attachment_type, &attachment_id)
            })
            .collect()
    }
}
