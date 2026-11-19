use std::collections::HashMap;

use async_trait::async_trait;
use readings_core::{Block, CatalogError, SceneLink, SceneTextCatalog};
use sqlx::migrate::Migrator;
use sqlx::{Row, SqlitePool};
use time::OffsetDateTime;
use transacting::Transaction;
use transacting::sqlite::connection;

use crate::kept;

const KEEP: &str = "
    INSERT INTO scene_texts (scene, version, blocks)
    SELECT ?1, ?2, ?3
    WHERE NOT EXISTS (SELECT 1 FROM scene_text_tombstones WHERE scene = ?1)
    ON CONFLICT (scene) DO UPDATE
    SET version = excluded.version, blocks = excluded.blocks
    WHERE scene_texts.version < excluded.version
";

const LAY_TOMBSTONE: &str =
    "INSERT INTO scene_text_tombstones (scene, laid_at) VALUES (?1, ?2) ON CONFLICT DO NOTHING";

const FORGET: &str = "DELETE FROM scene_texts WHERE scene = ?1";

const FORGET_OLD_TOMBSTONES: &str = "
    DELETE FROM scene_text_tombstones WHERE scene IN (
        SELECT scene FROM scene_text_tombstones
        WHERE laid_at < ?1
        ORDER BY laid_at, scene
        LIMIT ?2
    )
";

const TEXTS_OF: &str =
    "SELECT scene, blocks FROM scene_texts WHERE scene IN (SELECT value FROM json_each(?1))";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations/sqlite")
}

pub struct SqliteSceneTextCatalog {
    pool: SqlitePool,
}

impl SqliteSceneTextCatalog {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

pub(crate) fn in_microseconds(at: OffsetDateTime) -> i64 {
    i64::try_from(at.unix_timestamp_nanos() / 1_000).expect("a moment fits a signed 64-bit column")
}

pub(crate) fn unreachable(failure: impl std::error::Error + Send + Sync + 'static) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl SceneTextCatalog for SqliteSceneTextCatalog {
    async fn keep(
        &self,
        transaction: &mut Transaction,
        scene: &SceneLink,
        blocks: &[Block],
        version: u64,
    ) -> Result<bool, CatalogError> {
        let kept = sqlx::query(KEEP)
            .bind(scene.as_str())
            .bind(i64::try_from(version).map_err(unreachable)?)
            .bind(kept::scene_texts::to_json(blocks))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;

        Ok(kept.rows_affected() > 0)
    }

    async fn forget_scene(
        &self,
        transaction: &mut Transaction,
        scene: &SceneLink,
        at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        sqlx::query(LAY_TOMBSTONE)
            .bind(scene.as_str())
            .bind(in_microseconds(at))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?;
        sqlx::query(FORGET)
            .bind(scene.as_str())
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
        Ok(sqlx::query(FORGET_OLD_TOMBSTONES)
            .bind(in_microseconds(before))
            .bind(i64::try_from(at_most).unwrap_or(i64::MAX))
            .execute(connection(transaction).map_err(unreachable)?)
            .await
            .map_err(unreachable)?
            .rows_affected())
    }

    async fn texts_of(
        &self,
        scenes: &[SceneLink],
    ) -> Result<HashMap<SceneLink, Vec<Block>>, CatalogError> {
        let asked: Vec<&str> = scenes.iter().map(SceneLink::as_str).collect();
        let asked = serde_json::to_string(&asked).expect("names are plain data");

        sqlx::query(TEXTS_OF)
            .bind(asked)
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
            .iter()
            .map(|row| {
                let scene: String = row.try_get("scene").map_err(unreachable)?;
                let blocks: String = row.try_get("blocks").map_err(unreachable)?;

                Ok((
                    SceneLink::from(scene),
                    kept::scene_texts::from_json(&blocks)?,
                ))
            })
            .collect()
    }
}
