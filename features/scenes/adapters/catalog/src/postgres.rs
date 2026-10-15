use async_trait::async_trait;
use scenes_core::{CatalogError, ProjectLink, SceneCatalog, SceneId, SceneSummary, SceneTitle};
use sqlx::migrate::Migrator;
use sqlx::{PgPool, Row};
use time::OffsetDateTime;

const LEDGER: &str = "_sqlx_migrations_scene_catalog";

const REMEMBER: &str = "
    INSERT INTO scene_catalog (scene, project)
    SELECT $1, $2
    WHERE NOT EXISTS (SELECT 1 FROM scene_catalog_gone WHERE scene = $1)
    ON CONFLICT (scene) DO NOTHING
";

const RETITLE: &str = "
    INSERT INTO scene_catalog (scene, project, title, title_version)
    SELECT $1, $2, $3, $4
    WHERE NOT EXISTS (SELECT 1 FROM scene_catalog_gone WHERE scene = $1)
    ON CONFLICT (scene) DO UPDATE
    SET title = excluded.title, title_version = excluded.title_version
    WHERE scene_catalog.title_version < excluded.title_version
";

const NOTE_OPENING: &str = "
    INSERT INTO scene_catalog (scene, project, opening, opened_at)
    SELECT $1, $2, $3, $4
    WHERE NOT EXISTS (SELECT 1 FROM scene_catalog_gone WHERE scene = $1)
    ON CONFLICT (scene) DO UPDATE
    SET opening = excluded.opening, opened_at = excluded.opened_at
    WHERE scene_catalog.opened_at <= excluded.opened_at
";

const GONE: &str = "INSERT INTO scene_catalog_gone (scene) VALUES ($1) ON CONFLICT DO NOTHING";

const FORGET: &str = "DELETE FROM scene_catalog WHERE scene = $1";

const IN_PROJECT_AFTER: &str = "
    SELECT scene, project, title, opening
    FROM scene_catalog
    WHERE project = $1 AND scene > $2
    ORDER BY scene
    LIMIT $3
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
    async fn remember(&self, id: SceneId, project: &ProjectLink) -> Result<(), CatalogError> {
        sqlx::query(REMEMBER)
            .bind(id.to_string())
            .bind(project.to_string())
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn retitle(
        &self,
        id: SceneId,
        project: &ProjectLink,
        title: &SceneTitle,
        version: u64,
    ) -> Result<(), CatalogError> {
        sqlx::query(RETITLE)
            .bind(id.to_string())
            .bind(project.to_string())
            .bind(title.as_str())
            .bind(i64::try_from(version).expect("a version fits a signed 64-bit column"))
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn note_opening(
        &self,
        id: SceneId,
        project: &ProjectLink,
        opening: &str,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        sqlx::query(NOTE_OPENING)
            .bind(id.to_string())
            .bind(project.to_string())
            .bind(opening)
            .bind(in_microseconds(at))
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget(&self, id: SceneId) -> Result<(), CatalogError> {
        let mut transaction = self.pool.begin().await.map_err(unreachable)?;

        for gone in [GONE, FORGET] {
            sqlx::query(gone)
                .bind(id.to_string())
                .execute(&mut *transaction)
                .await
                .map_err(unreachable)?;
        }

        transaction.commit().await.map_err(unreachable)
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
}
