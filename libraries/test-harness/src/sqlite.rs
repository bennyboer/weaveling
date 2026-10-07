use std::fs;
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use std::time::Duration;

use sqlx::SqlitePool;
use sqlx::sqlite::{SqliteConnectOptions, SqliteJournalMode, SqlitePoolOptions};
use time::OffsetDateTime;

use crate::{is_stale, sanitized, stem};

const FIXTURES: &str = "weaveling-fixtures";
const POOLED: u32 = 5;
const BUSY_FOR: Duration = Duration::from_secs(5);

pub struct SqliteFixture {
    directory: PathBuf,
    handed_out: Mutex<Vec<SqlitePool>>,
}

impl SqliteFixture {
    pub fn setup() -> Self {
        let now = OffsetDateTime::now_utc();
        drop_stale(&fixtures(), now);

        Self {
            directory: fixtures().join(stem(now)),
            handed_out: Mutex::new(Vec::new()),
        }
    }

    pub fn directory(&self) -> &Path {
        &self.directory
    }

    pub async fn create_database(&self, feature: &str) -> SqlitePool {
        fs::create_dir_all(&self.directory).expect("the fixture directory should be creatable");

        let pool = SqlitePoolOptions::new()
            .max_connections(POOLED)
            .connect_with(
                SqliteConnectOptions::new()
                    .filename(
                        self.directory
                            .join(format!("{}.sqlite", sanitized(feature))),
                    )
                    .create_if_missing(true)
                    .journal_mode(SqliteJournalMode::Wal)
                    .foreign_keys(true)
                    .busy_timeout(BUSY_FOR),
            )
            .await
            .expect("a fixture database should open");
        self.handed_out
            .lock()
            .expect("the fixture's pool list is never poisoned")
            .push(pool.clone());

        pool
    }

    pub async fn cleanup(self) {
        let handed_out = self
            .handed_out
            .into_inner()
            .expect("the fixture's pool list is never poisoned");
        for pool in handed_out {
            pool.close().await;
        }

        let _ = fs::remove_dir_all(&self.directory);
    }
}

fn fixtures() -> PathBuf {
    std::env::temp_dir().join(FIXTURES)
}

fn drop_stale(fixtures: &Path, now: OffsetDateTime) {
    let Ok(known) = fs::read_dir(fixtures) else {
        return;
    };

    for left in known.flatten() {
        if left
            .file_name()
            .to_str()
            .is_some_and(|name| is_stale(name, now))
        {
            let _ = fs::remove_dir_all(left.path());
        }
    }
}
