use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;

use crate::postgres::{PostgresOutlineCatalog, migrations};
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    store: PostgresOutlineCatalog,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresOutlineCatalog;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("outline").await;
        migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            store: PostgresOutlineCatalog::new(pool),
        }
    }

    fn store(&self) -> &Self::Store {
        &self.store
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

crate::conformance_tests!(OnPostgres);
