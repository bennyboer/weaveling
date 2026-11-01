use async_trait::async_trait;
use eventsourcing::Version;
use ideas_core::{CatalogError, IdeaCatalog, IdeaId, IdeaSummary, IdeaTitle, ProjectLink};
use sqlx::migrate::Migrator;
use sqlx::postgres::PgRow;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

const REMEMBER: &str = "
    INSERT INTO idea_summaries (idea, version, project, title)
    SELECT $1, $2, $3, $4
    WHERE NOT EXISTS (SELECT 1 FROM idea_tombstones WHERE idea = $1)
    ON CONFLICT (idea) DO UPDATE
    SET version = EXCLUDED.version,
        project = EXCLUDED.project,
        title   = EXCLUDED.title
    WHERE idea_summaries.version < EXCLUDED.version
";

const FORGET: &str = "DELETE FROM idea_summaries WHERE idea = $1";

const LAY_TOMBSTONE: &str =
    "INSERT INTO idea_tombstones (idea, laid_at) VALUES ($1, $2) ON CONFLICT DO NOTHING";

const FORGET_OLD_SUMMARY_TOMBSTONES: &str = "
    DELETE FROM idea_tombstones WHERE idea IN (
        SELECT idea FROM idea_tombstones
        WHERE laid_at < $1
        ORDER BY laid_at, idea
        LIMIT $2
    )
";

const HOLD: &str = "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))";

const IN_PROJECT_AFTER: &str = "
    SELECT idea, version, project, title
    FROM idea_summaries
    WHERE project = $1 AND idea > $2
    ORDER BY idea
    LIMIT $3
";

const IN_PROJECT: &str = "
    SELECT idea, version, project, title
    FROM idea_summaries
    WHERE project = $1
    ORDER BY idea DESC
";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations/postgres")
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

    Ok(IdeaSummary {
        id: idea.parse().map_err(unreachable)?,
        version: Version::of(u64::try_from(version).map_err(unreachable)?),
        project: ProjectLink::from(project.as_str()),
        title: IdeaTitle::new(&title).map_err(unreachable)?,
    })
}

#[async_trait]
impl IdeaCatalog for PostgresIdeaCatalog {
    async fn remember(&self, summary: &IdeaSummary) -> Result<(), CatalogError> {
        let mut transaction = self.pool.begin().await.map_err(unreachable)?;
        sqlx::query(HOLD)
            .bind(summary.id.to_string())
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;
        sqlx::query(REMEMBER)
            .bind(summary.id.to_string())
            .bind(summary.version.count() as i64)
            .bind(summary.project.to_string())
            .bind(summary.title.as_str())
            .execute(&mut *transaction)
            .await
            .map_err(unreachable)?;

        transaction.commit().await.map_err(unreachable)
    }

    async fn forget(&self, id: &IdeaId, at: OffsetDateTime) -> Result<(), CatalogError> {
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
