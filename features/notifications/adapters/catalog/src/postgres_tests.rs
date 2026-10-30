use async_trait::async_trait;
use test_harness::PostgresFixture;

use crate::postgres;
use crate::postgres::PostgresNoticeCatalog;
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    store: PostgresNoticeCatalog,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresNoticeCatalog;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("notifications").await;
        postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            store: PostgresNoticeCatalog::new(pool),
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
