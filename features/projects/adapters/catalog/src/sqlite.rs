use async_trait::async_trait;
use clock::text;
use eventsourcing::Version;
use projects_core::{CatalogError, ProjectCatalog, ProjectId, ProjectName, ProjectSummary};
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};

const REMEMBER: &str = "
    INSERT INTO project_summaries (project, version, name, created_at, updated_at)
    VALUES (?1, ?2, ?3, ?4, ?5)
    ON CONFLICT (project) DO UPDATE
    SET version    = excluded.version,
        name       = excluded.name,
        created_at = excluded.created_at,
        updated_at = excluded.updated_at
";

const FORGET: &str = "DELETE FROM project_summaries WHERE project = ?1";

const ALL: &str = "
    SELECT project, version, name, created_at, updated_at
    FROM project_summaries
    ORDER BY project DESC
";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations/sqlite")
}

pub struct SqliteProjectCatalog {
    pool: SqlitePool,
}

impl SqliteProjectCatalog {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl std::error::Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

fn unreadable(column: &str, written: &str) -> CatalogError {
    CatalogError::Backend(format!("{column} holds {written:?}, which is no time").into())
}

fn to_summary(row: &SqliteRow) -> Result<ProjectSummary, CatalogError> {
    let project: String = row.try_get("project").map_err(unreachable)?;
    let version: i64 = row.try_get("version").map_err(unreachable)?;
    let name: String = row.try_get("name").map_err(unreachable)?;
    let created_at: String = row.try_get("created_at").map_err(unreachable)?;
    let updated_at: String = row.try_get("updated_at").map_err(unreachable)?;

    Ok(ProjectSummary {
        id: project.parse().map_err(unreachable)?,
        version: Version::of(u64::try_from(version).map_err(unreachable)?),
        name: ProjectName::new(&name).map_err(unreachable)?,
        created_at: text::read(&created_at).ok_or_else(|| unreadable("created_at", &created_at))?,
        updated_at: text::read(&updated_at).ok_or_else(|| unreadable("updated_at", &updated_at))?,
    })
}

#[async_trait]
impl ProjectCatalog for SqliteProjectCatalog {
    async fn remember(&self, summary: &ProjectSummary) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(summary.version.count() as i64)
            .bind(summary.name.as_str())
            .bind(text::written(summary.created_at))
            .bind(text::written(summary.updated_at))
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget(&self, id: &ProjectId) -> Result<(), CatalogError> {
        sqlx::query(FORGET)
            .bind(id.to_string())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn all(&self) -> Result<Vec<ProjectSummary>, CatalogError> {
        let found = sqlx::query(ALL)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found.iter().map(to_summary).collect()
    }
}
