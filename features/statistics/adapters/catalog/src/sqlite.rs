use async_trait::async_trait;
use sqlx::migrate::Migrator;
use sqlx::sqlite::SqliteRow;
use sqlx::{Row, SqlitePool};
use statistics_core::{CatalogError, ProjectLink, SceneLink, WordCount, WordCountCatalog};
use time::OffsetDateTime;

const KEEP: &str = "
    INSERT INTO word_counts (scene, project, words, counted_at)
    SELECT ?1, ?2, ?3, ?4
    WHERE NOT EXISTS (SELECT 1 FROM gone_scenes WHERE scene = ?1)
    ON CONFLICT (scene) DO UPDATE
    SET project = excluded.project, words = excluded.words, counted_at = excluded.counted_at
    WHERE word_counts.counted_at <= excluded.counted_at
";

const GONE: &str = "INSERT INTO gone_scenes (scene) VALUES (?1) ON CONFLICT DO NOTHING";

const FORGET: &str = "DELETE FROM word_counts WHERE scene = ?1";

const BEGIN_WRITING: &str = "BEGIN IMMEDIATE";

const IN_PROJECT: &str = "SELECT scene, words FROM word_counts WHERE project = ?1 ORDER BY scene";

pub fn migrations() -> Migrator {
    sqlx::migrate!("./migrations/sqlite")
}

pub struct SqliteWordCountCatalog {
    pool: SqlitePool,
}

impl SqliteWordCountCatalog {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

fn in_microseconds(at: OffsetDateTime) -> i64 {
    i64::try_from(at.unix_timestamp_nanos() / 1_000).expect("a moment fits a signed 64-bit column")
}

fn count_from(row: &SqliteRow) -> Result<WordCount, CatalogError> {
    let scene: String = row.try_get("scene").map_err(unreachable)?;
    let words: i64 = row.try_get("words").map_err(unreachable)?;

    Ok(WordCount {
        scene: SceneLink::from(scene),
        words: u64::try_from(words).map_err(|why| CatalogError::Backend(Box::new(why)))?,
    })
}

fn unreachable(failure: sqlx::Error) -> CatalogError {
    CatalogError::Backend(Box::new(failure))
}

#[async_trait]
impl WordCountCatalog for SqliteWordCountCatalog {
    async fn keep(
        &self,
        scene: &SceneLink,
        project: &ProjectLink,
        words: u64,
        counted_at: OffsetDateTime,
    ) -> Result<(), CatalogError> {
        sqlx::query(KEEP)
            .bind(scene.as_str())
            .bind(project.as_str())
            .bind(i64::try_from(words).expect("a word count fits a signed 64-bit column"))
            .bind(in_microseconds(counted_at))
            .execute(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn forget_scene(&self, scene: &SceneLink) -> Result<(), CatalogError> {
        let mut transaction = self
            .pool
            .begin_with(BEGIN_WRITING)
            .await
            .map_err(unreachable)?;

        for gone in [GONE, FORGET] {
            sqlx::query(gone)
                .bind(scene.as_str())
                .execute(&mut *transaction)
                .await
                .map_err(unreachable)?;
        }

        transaction.commit().await.map_err(unreachable)
    }

    async fn in_project(&self, project: &ProjectLink) -> Result<Vec<WordCount>, CatalogError> {
        sqlx::query(IN_PROJECT)
            .bind(project.as_str())
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?
            .iter()
            .map(count_from)
            .collect()
    }
}
