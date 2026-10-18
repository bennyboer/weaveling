use async_trait::async_trait;
use sqlx::SqlitePool;
use test_harness::SqliteFixture;

use crate::sqlite::{SqliteQuotationRestylingTasks, SqliteSceneCatalog, migrations};
use crate::suite::Workbench;

struct OnSqlite {
    fixture: SqliteFixture,
    catalog: SqliteSceneCatalog,
    tasks: SqliteQuotationRestylingTasks,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Catalog = SqliteSceneCatalog;
    type Tasks = SqliteQuotationRestylingTasks;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool: SqlitePool = fixture.create_database("scenes").await;
        migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            catalog: SqliteSceneCatalog::new(pool.clone()),
            tasks: SqliteQuotationRestylingTasks::new(pool),
        }
    }

    fn catalog(&self) -> &Self::Catalog {
        &self.catalog
    }

    fn tasks(&self) -> &Self::Tasks {
        &self.tasks
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

crate::suite::conformance_tests!(OnSqlite);
