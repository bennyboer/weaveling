use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;

use crate::postgres::{PostgresProjectStore, migrations};
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    store: PostgresProjectStore,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresProjectStore;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("projects").await;
        migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            store: PostgresProjectStore::new(pool),
        }
    }

    fn store(&self) -> &Self::Store {
        &self.store
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

crate::suite::conformance_tests!(OnPostgres);
