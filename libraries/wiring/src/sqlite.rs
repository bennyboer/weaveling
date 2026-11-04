use std::fs;
use std::path::{Path, PathBuf};
use std::time::Duration;

use async_trait::async_trait;
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};

use crate::Unprepared;

const POOLED: u32 = 5;
const BUSY_FOR: Duration = Duration::from_secs(5);

#[async_trait]
pub trait Files: Send + Sync {
    async fn ready(&self, feature: &str) -> Result<SqlitePool, Unprepared>;
}

pub struct DataDirectory {
    directory: PathBuf,
}

impl DataDirectory {
    pub fn at(directory: impl Into<PathBuf>) -> Self {
        Self {
            directory: directory.into(),
        }
    }
}

#[async_trait]
impl Files for DataDirectory {
    async fn ready(&self, feature: &str) -> Result<SqlitePool, Unprepared> {
        connect(&self.directory, feature).await
    }
}

pub fn file_of(directory: &Path, feature: &str) -> PathBuf {
    directory.join(format!("{feature}.sqlite"))
}

pub async fn connect(directory: &Path, feature: &str) -> Result<SqlitePool, Unprepared> {
    let file = file_of(directory, feature);

    fs::create_dir_all(directory).map_err(|why| Unprepared::Uncreatable {
        database: file.display().to_string(),
        why: why.to_string(),
    })?;

    SqlitePoolOptions::new()
        .max_connections(POOLED)
        .connect_with(
            SqliteConnectOptions::new()
                .filename(&file)
                .create_if_missing(true)
                .journal_mode(SqliteJournalMode::Wal)
                .foreign_keys(true)
                .busy_timeout(BUSY_FOR),
        )
        .await
        .map_err(|why| Unprepared::Unreachable {
            database: file.display().to_string(),
            why: why.to_string(),
        })
}

pub async fn lay_out(
    feature: &str,
    pool: &SqlitePool,
    migrations: Migrator,
) -> Result<(), Unprepared> {
    migrations
        .run(pool)
        .await
        .map_err(|why| Unprepared::Unmigrated {
            database: feature.to_owned(),
            why: why.to_string(),
        })
}

#[cfg(test)]
mod tests {
    use std::time::{SystemTime, UNIX_EPOCH};

    use super::*;

    fn a_directory(named: &str) -> PathBuf {
        let now = SystemTime::now()
            .duration_since(UNIX_EPOCH)
            .expect("the clock is past 1970")
            .as_nanos();

        std::env::temp_dir().join(format!("weaveling-wiring-{named}-{now}"))
    }

    async fn tidy(pool: SqlitePool, directory: &Path) {
        pool.close().await;
        let _ = fs::remove_dir_all(directory);
    }

    #[tokio::test]
    async fn a_feature_keeps_its_own_file_in_the_data_directory() {
        // given: a data directory
        let directory = a_directory("file");

        // when: a feature connects
        let pool = connect(&directory, "ideas").await.expect("should connect");

        // then: it has a file of its own there
        assert!(
            directory.join("ideas.sqlite").is_file(),
            "one file per feature, so a feature's data stays unwritable by any other"
        );
        tidy(pool, &directory).await;
    }

    #[tokio::test]
    async fn a_data_directory_that_does_not_exist_yet_is_made() {
        // given: a data directory that does not exist yet
        let directory = a_directory("nested").join("deeper").join("still");

        // when: a feature connects
        let pool = connect(&directory, "ideas").await.expect("should connect");

        // then: its file is made there
        assert!(directory.join("ideas.sqlite").is_file());
        tidy(pool, &directory).await;
    }

    #[tokio::test]
    async fn the_file_is_written_ahead_so_readers_never_wait_for_the_writer() {
        // given: a feature's database
        let directory = a_directory("wal");
        let pool = connect(&directory, "ideas").await.expect("should connect");

        // when: its journal mode is asked for
        let mode: String = sqlx::query_scalar("PRAGMA journal_mode")
            .fetch_one(&pool)
            .await
            .expect("should ask");

        // then: it is written ahead
        assert_eq!(mode, "wal");
        tidy(pool, &directory).await;
    }

    #[tokio::test]
    async fn foreign_keys_are_enforced_on_every_connection() {
        // given: a feature's database with a table referring to another
        let directory = a_directory("keys");
        let pool = connect(&directory, "scenes").await.expect("should connect");
        sqlx::raw_sql(
            "CREATE TABLE scenes (scene TEXT PRIMARY KEY);
             CREATE TABLE scene_ideas (scene TEXT NOT NULL REFERENCES scenes (scene));",
        )
        .execute(&pool)
        .await
        .expect("should create");

        // when: a row refers to nothing
        let orphan = sqlx::query("INSERT INTO scene_ideas (scene) VALUES ('nowhere')")
            .execute(&pool)
            .await;

        // then: it is refused
        assert!(
            orphan.is_err(),
            "SQLite ignores foreign keys unless every connection asks for them"
        );
        tidy(pool, &directory).await;
    }

    #[tokio::test]
    async fn a_schema_is_laid_down_once_however_often_it_is_asked_for() {
        // given: a feature's database and a schema for it
        let directory = a_directory("lay-out");
        let scripts = directory.join("migrations");
        fs::create_dir_all(&scripts).expect("should make the scripts directory");
        fs::write(
            scripts.join("0001_notes.sql"),
            "CREATE TABLE notes (note TEXT NOT NULL);",
        )
        .expect("should write the script");
        let pool = connect(&directory, "notes").await.expect("should connect");

        // when: the schema is laid down twice
        for _ in 0..2 {
            let migrator = Migrator::new(scripts.as_path())
                .await
                .expect("should read the scripts");
            lay_out("notes", &pool, migrator)
                .await
                .expect("laying out twice should be harmless");
        }

        // then: its table is there once
        let tables: i64 =
            sqlx::query_scalar("SELECT count(*) FROM sqlite_master WHERE name = 'notes'")
                .fetch_one(&pool)
                .await
                .expect("should count");
        assert_eq!(tables, 1);
        tidy(pool, &directory).await;
    }
}
