use async_trait::async_trait;
use outline_core::{
    Attachment, CatalogError, OutlineCatalog, OutlineId, OutlineSummary, ProjectLink,
};
use sqlx::migrate::Migrator;
use sqlx::{PgPool, Row};

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
    INSERT INTO outline_attachments (outline, kind, attached)
    SELECT $1, kind, attached
    FROM unnest($2::text[], $3::text[]) AS held (kind, attached)
    ON CONFLICT DO NOTHING
";

const HOLDING: &str = "
    SELECT outline
    FROM outline_attachments
    WHERE kind = $1 AND attached = $2
    ORDER BY outline
";

fn kind_of(attachment: &Attachment) -> &'static str {
    match attachment {
        Attachment::Passage(_) => "passage",
        Attachment::Idea(_) => "idea",
    }
}

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations")
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
        let kinds: Vec<&str> = attachments.iter().map(kind_of).collect();
        let held: Vec<String> = attachments.iter().map(ToString::to_string).collect();
        let mut transaction = self.pool.begin().await.map_err(unreachable)?;

        sqlx::query(LET_GO)
            .bind(outline.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        sqlx::query(HOLD)
            .bind(outline.to_string())
            .bind(&kinds)
            .bind(&held)
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
            .bind(kind_of(attachment))
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
}
