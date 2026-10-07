use async_trait::async_trait;
use eventsourcing::Version;
use ideas_core::{CatalogError, IdeaCatalog, IdeaId, IdeaSummary, IdeaTitle, ProjectLink};
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

const REMEMBER: &str = "
    INSERT INTO idea_summaries (idea, version, project, title)
    VALUES (?1, ?2, ?3, ?4)
    ON CONFLICT (idea) DO UPDATE
    SET version = excluded.version,
        project = excluded.project,
        title   = excluded.title
";

const FORGET: &str = "DELETE FROM idea_summaries WHERE idea = ?1";

const IN_PROJECT_AFTER: &str = "
    SELECT idea, version, project, title
    FROM idea_summaries
    WHERE project = ?1 AND idea > ?2
    ORDER BY idea
    LIMIT ?3
";

const IN_PROJECT: &str = "
    SELECT idea, version, project, title
    FROM idea_summaries
    WHERE project = ?1
    ORDER BY idea DESC
";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations/sqlite")
}

pub struct SqliteIdeaCatalog {
    pool: SqlitePool,
}

impl SqliteIdeaCatalog {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl std::error::Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

fn to_summary(row: &SqliteRow) -> Result<IdeaSummary, CatalogError> {
    let idea: String = row.try_get("idea").map_err(unreachable)?;
    let version: i64 = row.try_get("version").map_err(unreachable)?;
    let project: String = row.try_get("project").map_err(unreachable)?;
    let title: String = row.try_get("title").map_err(unreachable)?;

    Ok(IdeaSummary {
        id: idea.parse().map_err(unreachable)?,
        version: Version::of(u64::try_from(version).map_err(unreachable)?),
        project: ProjectLink::from(project.as_str()),
        title: IdeaTitle::new(&title).map_err(unreachable)?,
    })
}

#[async_trait]
impl IdeaCatalog for SqliteIdeaCatalog {
    async fn remember(&self, summary: &IdeaSummary) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(summary.version.count() as i64)
            .bind(summary.project.to_string())
            .bind(summary.title.as_str())
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
