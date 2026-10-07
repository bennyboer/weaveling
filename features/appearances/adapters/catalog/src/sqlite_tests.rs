use async_trait::async_trait;
use sqlx::SqlitePool;
use test_harness::SqliteFixture;

use crate::sqlite::{SqliteAppearanceCatalog, migrations};
use crate::suite::Workbench;

struct OnSqlite {
    fixture: SqliteFixture,
    catalog: SqliteAppearanceCatalog,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Catalog = SqliteAppearanceCatalog;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool: SqlitePool = fixture.create_database("appearances").await;
        migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty file");

        Self {
            fixture,
            catalog: SqliteAppearanceCatalog::new(pool),
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
