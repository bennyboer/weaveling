use async_trait::async_trait;
use scenes_core::{
    CatalogError, Page, ProjectLink, QuotationRestylingProgress, QuotationRestylingStanding,
    QuotationRestylingTaskError, QuotationRestylingTasks, SceneCatalog, SceneId, SceneSummary,
    SceneTitle,
};
use sqlx::migrate::Migrator;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;
use transacting::Transaction;
use transacting::postgres::connection;

const LEDGER: &str = "_sqlx_migrations_scene_catalog";

const REMEMBER: &str = "
    INSERT INTO scene_catalog (scene, project)
    SELECT $1, $2
    WHERE NOT EXISTS (SELECT 1 FROM scene_tombstones WHERE scene = $1)
    ON CONFLICT (scene) DO NOTHING
";

const RETITLE: &str = "
    INSERT INTO scene_catalog (scene, project, title, title_version)
    SELECT $1, $2, $3, $4
    WHERE NOT EXISTS (SELECT 1 FROM scene_tombstones WHERE scene = $1)
    ON CONFLICT (scene) DO UPDATE
    SET title = excluded.title, title_version = excluded.title_version
    WHERE scene_catalog.title_version < excluded.title_version
";

const NOTE_OPENING: &str = "
    INSERT INTO scene_catalog (scene, project, opening, opened_at)
    SELECT $1, $2, $3, $4
    WHERE NOT EXISTS (SELECT 1 FROM scene_tombstones WHERE scene = $1)
    ON CONFLICT (scene) DO UPDATE
    SET opening = excluded.opening, opened_at = excluded.opened_at
    WHERE scene_catalog.opened_at <= excluded.opened_at
";

const LAY_TOMBSTONE: &str =
    "INSERT INTO scene_tombstones (scene, laid_at) VALUES ($1, $2) ON CONFLICT DO NOTHING";

const FORGET_OLD_TOMBSTONES: &str = "
    DELETE FROM scene_tombstones WHERE scene IN (
        SELECT scene FROM scene_tombstones
        WHERE laid_at < $1
        ORDER BY laid_at, scene
        LIMIT $2
    )
";

const LAY_PROJECT_TOMBSTONE: &str =
    "INSERT INTO project_tombstones (project, laid_at) VALUES ($1, $2) ON CONFLICT DO NOTHING";

const HAS_PROJECT_TOMBSTONE: &str =
    "SELECT EXISTS (SELECT 1 FROM project_tombstones WHERE project = $1)";

const FORGET_OLD_PROJECT_TOMBSTONES: &str = "
    DELETE FROM project_tombstones WHERE project IN (
        SELECT project FROM project_tombstones
        WHERE laid_at < $1
        ORDER BY laid_at, project
        LIMIT $2
    )
";

const SUMMARY_OF: &str = "
    SELECT scene, project, title, opening FROM scene_catalog WHERE scene = $1
";

const FORGET: &str = "DELETE FROM scene_catalog WHERE scene = $1";

const IN_PROJECT_AFTER: &str = "
    SELECT scene, project, title, opening
    FROM scene_catalog
    WHERE project = $1 AND scene > $2
    ORDER BY scene
    LIMIT $3
";

const BEGIN: &str = "
    INSERT INTO quotation_restyling_tasks (project, latest_change, total, cursor)
    VALUES ($1, $2, $3, $4)
    ON CONFLICT (project) DO UPDATE
    SET latest_change = excluded.latest_change, total = excluded.total, done = 0, finished = FALSE, cursor = excluded.cursor
    WHERE quotation_restyling_tasks.latest_change < excluded.latest_change
";

const STANDING: &str = "
    SELECT latest_change, cursor, finished
    FROM quotation_restyling_tasks
    WHERE project = $1
";

const PROGRESSED: &str = "
    UPDATE quotation_restyling_tasks
    SET done = done + $5, finished = $6, cursor = $4
    WHERE project = $1 AND latest_change = $2 AND cursor IS NOT DISTINCT FROM $3 AND finished = FALSE
";

const PROGRESS: &str = "
    SELECT latest_change, total, done, finished
    FROM quotation_restyling_tasks
    WHERE project = $1
";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/postgres");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct PostgresSceneCatalog {
    pool: PgPool,
}

impl PostgresSceneCatalog {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn in_microseconds(at: OffsetDateTime) -> i64 {
    i64::try_from(at.unix_timestamp_nanos() / 1_000).expect("a moment fits a signed 64-bit column")
}

fn unreachable(failure: impl std::error::Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

fn summary_from(row: &sqlx::postgres::PgRow) -> Result<SceneSummary, CatalogError> {
    let scene: String = row.try_get("scene").map_err(unreachable)?;
    let project: String = row.try_get("project").map_err(unreachable)?;
    let title: String = row.try_get("title").map_err(unreachable)?;
    let opening: String = row.try_get("opening").map_err(unreachable)?;

    Ok(SceneSummary {
        id: scene.parse().map_err(unreachable)?,
        project: ProjectLink::from(project),
        title: SceneTitle::new(&title).map_err(unreachable)?,
        opening,
    })
}

#[async_trait]
impl SceneCatalog for PostgresSceneCatalog {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        project: &ProjectLink,
    ) -> Result<(), CatalogError> {
        transacting::postgres::one_at_a_time(transaction, &id.to_string())
            .await
            .map_err(unreachable)?;
        sqlx::query(REMEMBER)
            .bind(id.to_string())
            .bind(project.to_string())
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn retitle(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        project: &ProjectLink,
        title: &SceneTitle,
        version: u64,
    ) -> Result<(), CatalogError> {
        transacting::postgres::one_at_a_time(transaction, &id.to_string())
            .await
            .map_err(unreachable)?;
        sqlx::query(RETITLE)
            .bind(id.to_string())
            .bind(project.to_string())
            .bind(title.as_str())
            .bind(i64::try_from(version).expect("a version fits a signed 64-bit column"))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn note_opening(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        project: &ProjectLink,
        opening: &str,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        transacting::postgres::one_at_a_time(transaction, &id.to_string())
            .await
            .map_err(unreachable)?;
        sqlx::query(NOTE_OPENING)
            .bind(id.to_string())
            .bind(project.to_string())
            .bind(opening)
            .bind(in_microseconds(at))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        transacting::postgres::one_at_a_time(transaction, &id.to_string())
            .await
            .map_err(unreachable)?;
        sqlx::query(LAY_TOMBSTONE)
            .bind(id.to_string())
            .bind(in_microseconds(at))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;
        sqlx::query(FORGET)
            .bind(id.to_string())
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
        let scenes = sqlx::query(FORGET_OLD_TOMBSTONES)
            .bind(in_microseconds(before))
            .bind(i64::try_from(at_most).unwrap_or(i64::MAX))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?
            .rows_affected();
        let projects = sqlx::query(FORGET_OLD_PROJECT_TOMBSTONES)
            .bind(in_microseconds(before))
            .bind(i64::try_from(at_most).unwrap_or(i64::MAX))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?
            .rows_affected();

        Ok(scenes + projects)
    }

    async fn summary_of(&self, id: SceneId) -> Result<Option<SceneSummary>, CatalogError> {
        sqlx::query(SUMMARY_OF)
            .bind(id.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(unreachable)?
            .as_ref()
            .map(summary_from)
            .transpose()
    }

    async fn in_project_after(
        &self,
        project: &ProjectLink,
        after: Option<SceneId>,
        at_most: usize,
    ) -> Result<Vec<SceneSummary>, CatalogError> {
        sqlx::query(IN_PROJECT_AFTER)
            .bind(project.to_string())
            .bind(after.map(|last| last.to_string()).unwrap_or_default())
            .bind(i64::try_from(at_most).expect("a page size fits a signed 64-bit column"))
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
            .iter()
            .map(summary_from)
            .collect()
    }

    async fn lay_project_tombstone(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        sqlx::query(LAY_PROJECT_TOMBSTONE)
            .bind(project.to_string())
            .bind(in_microseconds(at))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn has_project_tombstone(&self, project: &ProjectLink) -> Result<bool, CatalogError> {
        sqlx::query_scalar(HAS_PROJECT_TOMBSTONE)
            .bind(project.to_string())
            .fetch_one(&self.pool)
            .await
            .map_err(unreachable)
    }
}

pub struct PostgresQuotationRestylingTasks {
    pool: PgPool,
}

impl PostgresQuotationRestylingTasks {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

fn untold(failure: impl std::error::Error + Send + Sync + 'static) -> QuotationRestylingTaskError {
    QuotationRestylingTaskError::Backend(Box::new(failure))
}

fn in_column(count: u64) -> i64 {
    i64::try_from(count).expect("a count fits a signed 64-bit column")
}

fn counted(row: &sqlx::postgres::PgRow, column: &str) -> Result<u64, QuotationRestylingTaskError> {
    let stored: i64 = row.try_get(column).map_err(untold)?;

    u64::try_from(stored).map_err(untold)
}

#[async_trait]
impl QuotationRestylingTasks for PostgresQuotationRestylingTasks {
    async fn begin(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        change: u64,
        total: u64,
        cursor: Option<&str>,
    ) -> Result<bool, QuotationRestylingTaskError> {
        let written = sqlx::query(BEGIN)
            .bind(project.to_string())
            .bind(in_column(change))
            .bind(in_column(total))
            .bind(cursor)
            .execute(connection(transaction).map_err(untold)?)
            .await
            .map_err(untold)?;

        Ok(written.rows_affected() == 1)
    }

    async fn standing(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
    ) -> Result<Option<QuotationRestylingStanding>, QuotationRestylingTaskError> {
        let Some(row) = sqlx::query(STANDING)
            .bind(project.to_string())
            .fetch_optional(connection(transaction).map_err(untold)?)
            .await
            .map_err(untold)?
        else {
            return Ok(None);
        };

        Ok(Some(QuotationRestylingStanding {
            change: counted(&row, "latest_change")?,
            cursor: row.try_get("cursor").map_err(untold)?,
            finished: row.try_get("finished").map_err(untold)?,
        }))
    }

    async fn progressed(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        change: u64,
        page: Page<'_>,
        finished: bool,
    ) -> Result<bool, QuotationRestylingTaskError> {
        let written = sqlx::query(PROGRESSED)
            .bind(project.to_string())
            .bind(in_column(change))
            .bind(page.from)
            .bind(page.to)
            .bind(in_column(page.done))
            .bind(finished)
            .execute(connection(transaction).map_err(untold)?)
            .await
            .map_err(untold)?;

        Ok(written.rows_affected() == 1)
    }

    async fn progress(
        &self,
        project: &ProjectLink,
    ) -> Result<Option<QuotationRestylingProgress>, QuotationRestylingTaskError> {
        let Some(row) = sqlx::query(PROGRESS)
            .bind(project.to_string())
            .fetch_optional(&self.pool)
            .await
            .map_err(untold)?
        else {
            return Ok(None);
        };

        Ok(Some(QuotationRestylingProgress {
            change: counted(&row, "latest_change")?,
            total: counted(&row, "total")?,
            done: counted(&row, "done")?,
            finished: row.try_get("finished").map_err(untold)?,
        }))
    }
}
