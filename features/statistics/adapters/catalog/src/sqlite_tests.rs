use async_trait::async_trait;
use sqlx::SqlitePool;
use test_harness::SqliteFixture;

use crate::sqlite;
use crate::sqlite::SqliteWordCountCatalog;
use crate::suite::Workbench;

struct OnSqlite {
    fixture: SqliteFixture,
    catalog: SqliteWordCountCatalog,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Catalog = SqliteWordCountCatalog;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool: SqlitePool = fixture.create_database("statistics").await;
        sqlite::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            catalog: SqliteWordCountCatalog::new(pool),
        }
    }

    fn catalog(&self) -> &Self::Catalog {
        &self.catalog
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

crate::suite::conformance_tests!(OnSqlite);
