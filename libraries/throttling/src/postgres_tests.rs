use async_trait::async_trait;
use test_harness::PostgresFixture;

use crate::postgres;
use crate::postgres::PostgresThrottles;
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    store: PostgresThrottles,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresThrottles;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("throttles").await;
        postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down where nothing is yet");

        Self {
            fixture,
            store: PostgresThrottles::new(pool),
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
