use async_trait::async_trait;
use test_harness::SqliteFixture;

use crate::sqlite;
use crate::sqlite::SqliteNoticeCatalog;
use crate::suite::Workbench;

struct OnSqlite {
    fixture: SqliteFixture,
    store: SqliteNoticeCatalog,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Store = SqliteNoticeCatalog;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool = fixture.create_database("notifications").await;
        sqlite::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            store: SqliteNoticeCatalog::new(pool),
        }
    }

    fn store(&self) -> &Self::Store {
        &self.store
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

crate::conformance_tests!(OnSqlite);
