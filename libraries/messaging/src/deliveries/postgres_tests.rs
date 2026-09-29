use async_trait::async_trait;
use test_harness::PostgresFixture;

use crate::deliveries::postgres::{PostgresDeliveries, migrations};
use crate::deliveries::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    store: PostgresDeliveries,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresDeliveries;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("messaging").await;
        migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            store: PostgresDeliveries::new(pool),
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
