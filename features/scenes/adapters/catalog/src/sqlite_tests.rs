use async_trait::async_trait;
use sqlx::SqlitePool;
use test_harness::SqliteFixture;

use crate::sqlite::{SqliteSceneCatalog, migrations};
use crate::suite::Workbench;

struct OnSqlite {
    fixture: SqliteFixture,
    catalog: SqliteSceneCatalog,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Catalog = SqliteSceneCatalog;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool: SqlitePool = fixture.create_database("scenes").await;
        migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            catalog: SqliteSceneCatalog::new(pool),
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
