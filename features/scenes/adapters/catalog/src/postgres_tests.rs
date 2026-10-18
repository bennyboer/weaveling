use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;

use crate::postgres::{PostgresQuotationRestylingTasks, PostgresSceneCatalog, migrations};
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    catalog: PostgresSceneCatalog,
    tasks: PostgresQuotationRestylingTasks,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Catalog = PostgresSceneCatalog;
    type Tasks = PostgresQuotationRestylingTasks;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("scenes").await;
        migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            catalog: PostgresSceneCatalog::new(pool.clone()),
            tasks: PostgresQuotationRestylingTasks::new(pool),
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

crate::suite::conformance_tests!(OnPostgres);
