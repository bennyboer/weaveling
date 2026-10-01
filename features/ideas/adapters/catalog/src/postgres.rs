use async_trait::async_trait;
use eventsourcing::Version;
use ideas_core::{
    CatalogError, IdeaCatalog, IdeaId, IdeaSummary, IdeaTitle, PassageLink, ProjectLink,
};
use sqlx::migrate::Migrator;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};

const REMEMBER: &str = "
    INSERT INTO idea_summaries (idea, version, project, title, passage)
    VALUES ($1, $2, $3, $4, $5)
    ON CONFLICT (idea) DO UPDATE
    SET version = EXCLUDED.version,
        project = EXCLUDED.project,
        title   = EXCLUDED.title,
        passage = EXCLUDED.passage
";

const FORGET: &str = "DELETE FROM idea_summaries WHERE idea = $1";

const IN_PROJECT_AFTER: &str = "
    SELECT idea, version, project, title, passage
    FROM idea_summaries
    WHERE project = $1 AND idea > $2
    ORDER BY idea
    LIMIT $3
";

const IN_PROJECT: &str = "
    SELECT idea, version, project, title, passage
    FROM idea_summaries
    WHERE project = $1
    ORDER BY idea DESC
";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations")
}

pub struct PostgresIdeaCatalog {
    pool: PgPool,
}

impl PostgresIdeaCatalog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl std::error::Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

fn to_summary(row: &PgRow) -> Result<IdeaSummary, CatalogError> {
    let idea: String = row.try_get("idea").map_err(unreachable)?;
    let version: i64 = row.try_get("version").map_err(unreachable)?;
    let project: String = row.try_get("project").map_err(unreachable)?;
    let title: String = row.try_get("title").map_err(unreachable)?;
    let passage: Option<String> = row.try_get("passage").map_err(unreachable)?;

    Ok(IdeaSummary {
        id: idea.parse().map_err(unreachable)?,
        version: Version::of(version as u64),
        project: ProjectLink::from(project.as_str()),
        title: IdeaTitle::new(&title).map_err(unreachable)?,
        passage: passage.map(|link| PassageLink::from(link.as_str())),
    })
}

#[async_trait]
impl IdeaCatalog for PostgresIdeaCatalog {
    async fn remember(&self, summary: &IdeaSummary) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(summary.version.count() as i64)
            .bind(summary.project.to_string())
            .bind(summary.title.as_str())
            .bind(summary.passage.as_ref().map(ToString::to_string))
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget(&self, id: &IdeaId) -> Result<(), CatalogError> {
        sqlx::query(FORGET)
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn in_project_after(
        &self,
        project: &ProjectLink,
        after: Option<IdeaId>,
        at_most: usize,
    ) -> Result<Vec<IdeaSummary>, CatalogError> {
        let found = sqlx::query(IN_PROJECT_AFTER)
            .bind(project.to_string())
            .bind(after.map(|last| last.to_string()).unwrap_or_default())
            .bind(at_most as i64)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found.iter().map(to_summary).collect()
    }

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<IdeaSummary>, CatalogError> {
        let found = sqlx::query(IN_PROJECT)
            .bind(project.to_string())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found.iter().map(to_summary).collect()
    }
}
