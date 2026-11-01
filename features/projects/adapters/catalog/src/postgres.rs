use async_trait::async_trait;
use eventsourcing::Version;
use projects_core::{CatalogError, ProjectCatalog, ProjectId, ProjectName, ProjectSummary};
use sqlx::migrate::Migrator;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

const REMEMBER: &str = "
    INSERT INTO project_summaries (project, version, name, created_at, updated_at)
    SELECT $1, $2, $3, $4, $5
    WHERE NOT EXISTS (SELECT 1 FROM project_tombstones WHERE project = $1)
    ON CONFLICT (project) DO UPDATE
    SET version    = EXCLUDED.version,
        name       = EXCLUDED.name,
        created_at = EXCLUDED.created_at,
        updated_at = EXCLUDED.updated_at
    WHERE project_summaries.version < EXCLUDED.version
";

const FORGET: &str = "DELETE FROM project_summaries WHERE project = $1";

const LAY_TOMBSTONE: &str =
    "INSERT INTO project_tombstones (project, laid_at) VALUES ($1, $2) ON CONFLICT DO NOTHING";

const FORGET_OLD_SUMMARY_TOMBSTONES: &str = "
    DELETE FROM project_tombstones WHERE project IN (
        SELECT project FROM project_tombstones
        WHERE laid_at < $1
        ORDER BY laid_at, project
        LIMIT $2
    )
";

const HOLD: &str = "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))";

const ALL: &str = "
    SELECT project, version, name, created_at, updated_at
    FROM project_summaries
    ORDER BY project DESC
";

const AMONG: &str = "
    SELECT project, version, name, created_at, updated_at
    FROM project_summaries
    WHERE project = ANY($1)
    ORDER BY project DESC
";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations/postgres")
}

pub struct PostgresProjectCatalog {
    pool: PgPool,
}

impl PostgresProjectCatalog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn unreachable(failure: impl std::error::Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

fn to_summary(row: &PgRow) -> Result<ProjectSummary, CatalogError> {
    let project: String = row.try_get("project").map_err(unreachable)?;
    let version: i64 = row.try_get("version").map_err(unreachable)?;
    let name: String = row.try_get("name").map_err(unreachable)?;
    let created_at: OffsetDateTime = row.try_get("created_at").map_err(unreachable)?;
    let updated_at: OffsetDateTime = row.try_get("updated_at").map_err(unreachable)?;

    Ok(ProjectSummary {
        id: project.parse().map_err(unreachable)?,
        version: Version::of(u64::try_from(version).map_err(unreachable)?),
        name: ProjectName::new(&name).map_err(unreachable)?,
        created_at,
        updated_at,
    })
}

#[async_trait]
impl ProjectCatalog for PostgresProjectCatalog {
    async fn remember(&self, summary: &ProjectSummary) -> Result<(), CatalogError> {
        let mut transaction = self.pool.begin().await.map_err(unreachable)?;
        sqlx::query(HOLD)
            .bind(summary.id.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;
        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(summary.version.count() as i64)
            .bind(summary.name.as_str())
            .bind(summary.created_at)
            .bind(summary.updated_at)
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        transaction.commit().await.map_err(unreachable)
    }

    async fn forget(&self, id: &ProjectId, at: OffsetDateTime) -> Result<(), CatalogError> {
        let mut transaction = self.pool.begin().await.map_err(unreachable)?;
        sqlx::query(HOLD)
            .bind(id.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;
        sqlx::query(LAY_TOMBSTONE)
            .bind(id.to_string())
            .bind(at)
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;
        sqlx::query(FORGET)
            .bind(id.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        transaction.commit().await.map_err(unreachable)
    }

    async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, CatalogError> {
        Ok(sqlx::query(FORGET_OLD_SUMMARY_TOMBSTONES)
            .bind(before)
            .bind(i64::try_from(at_most).unwrap_or(i64::MAX))
            .execute(&self.pool)
            .await
            .map_err(unreachable)?
            .rows_affected())
    }

    async fn all(&self) -> Result<Vec<ProjectSummary>, CatalogError> {
        let found = sqlx::query(ALL)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found.iter().map(to_summary).collect()
    }

    async fn among(&self, ids: &[ProjectId]) -> Result<Vec<ProjectSummary>, CatalogError> {
        let found = sqlx::query(AMONG)
            .bind(ids.iter().map(ToString::to_string).collect::<Vec<String>>())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        found.iter().map(to_summary).collect()
    }
}
