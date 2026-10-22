#![cfg(feature = "sqlite")]

mod journeys;

use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use axum::http::StatusCode;
use axum_test::TestServer;
use clock::SystemClock;
use mailing::LoggingMailer;
use serde_json::{Value, json};
use sqlx::SqlitePool;
use test_harness::SqliteFixture;
use weaveling_service_api::{Accounts, Adapters, Storage, Unprepared, app};
use wiring::sqlite::{DataDirectory, Files};

use journeys::{Backend, DATABASES, Journey, id_in};

struct OnSqlite {
    fixture: SqliteFixture,
    files: FileEach,
}

#[derive(Clone)]
struct FileEach {
    pools: Arc<HashMap<&'static str, SqlitePool>>,
}

impl FileEach {
    fn of(&self, database: &str) -> &SqlitePool {
        self.pools
            .get(database)
            .unwrap_or_else(|| panic!("the fixture should have made a {database} file"))
    }
}

#[async_trait]
impl Files for FileEach {
    async fn ready(&self, feature: &str) -> Result<SqlitePool, Unprepared> {
        self.pools
            .get(feature)
            .cloned()
            .ok_or_else(|| Unprepared::Unreachable {
                database: feature.to_owned(),
                why: "the fixture made no file for it".to_owned(),
            })
    }
}

#[async_trait]
impl Backend for OnSqlite {
    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let mut pools = HashMap::new();
        for database in DATABASES {
            pools.insert(database, fixture.create_database(database).await);
        }

        Self {
            fixture,
            files: FileEach {
                pools: Arc::new(pools),
            },
        }
    }

    fn storage(&self) -> Storage {
        Storage::Sqlite(Arc::new(self.files.clone()))
    }

    async fn read(&self, database: &str, query: &'static str) -> Vec<String> {
        sqlx::query_scalar(query)
            .fetch_all(self.files.of(database))
            .await
            .expect("reading should succeed")
    }

    async fn count(&self, database: &str, query: &'static str) -> i64 {
        sqlx::query_scalar(query)
            .fetch_one(self.files.of(database))
            .await
            .expect("counting should succeed")
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

journeys!(OnSqlite);

struct Remembering {
    directory: DataDirectory,
    opened: Mutex<Vec<SqlitePool>>,
}

#[async_trait]
impl Files for Remembering {
    async fn ready(&self, feature: &str) -> Result<SqlitePool, Unprepared> {
        let pool = self.directory.ready(feature).await?;
        self.opened
            .lock()
            .expect("the list of opened files is never poisoned")
            .push(pool.clone());

        Ok(pool)
    }
}

impl Remembering {
    async fn close_everything(&self) {
        let opened = std::mem::take(
            &mut *self
                .opened
                .lock()
                .expect("the list of opened files is never poisoned"),
        );
        for pool in opened {
            pool.close().await;
        }
    }
}

#[tokio::test]
async fn what_was_written_survives_the_service_starting_again_from_its_files() {
    let fixture = SqliteFixture::setup();
    let files = Arc::new(Remembering {
        directory: DataDirectory::at(fixture.directory()),
        opened: Mutex::new(Vec::new()),
    });
    let local = || Storage::Sqlite(files.clone());

    let id = {
        let first = TestServer::new(app(Adapters::assembled(
            local(),
            Accounts::Off,
            Arc::new(LoggingMailer),
            Arc::new(SystemClock),
        )
        .await
        .expect("the first start should lay every file down")));

        id_in(
            &first
                .post("/api/projects")
                .json(&json!({ "name": "Outliving" }))
                .await
                .json(),
            "id",
        )
    };
    files.close_everything().await;

    let second = TestServer::new(app(Adapters::assembled(
        local(),
        Accounts::Off,
        Arc::new(LoggingMailer),
        Arc::new(SystemClock),
    )
    .await
    .expect("a second start should find every file already laid down")));
    let found = second.get(&format!("/api/projects/{id}")).await;

    found.assert_status_ok();
    assert_eq!(
        found.json::<Value>()["name"].as_str(),
        Some("Outliving"),
        "nothing carries over but the files, which is the whole promise of local mode"
    );

    drop(second);
    files.close_everything().await;
    fixture.cleanup().await;
}

#[tokio::test(flavor = "multi_thread")]
async fn a_server_on_sqlite_keeps_an_address_signed_up_in_the_accounts_file() {
    let journey = Journey::<OnSqlite>::with_accounts().await;

    journey
        .server
        .post("/api/accounts")
        .json(&json!({ "email": "ada@example.com", "name": "Ada", "password": "correct horse battery" }))
        .await
        .assert_status(StatusCode::ACCEPTED);

    assert_eq!(
        journey
            .backend
            .read("accounts", "SELECT email FROM account_email_addresses")
            .await,
        vec!["ada@example.com".to_owned()]
    );
    assert_eq!(
        journey
            .backend
            .count("accounts", "SELECT count(*) FROM events")
            .await,
        1
    );
    journey.end().await;
}
