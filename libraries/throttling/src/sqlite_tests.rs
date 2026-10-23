use async_trait::async_trait;
use test_harness::SqliteFixture;

use crate::sqlite;
use crate::sqlite::SqliteThrottles;
use crate::suite::Workbench;

struct OnSqlite {
    fixture: SqliteFixture,
    store: SqliteThrottles,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Store = SqliteThrottles;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool = fixture.create_database("throttles").await;
        sqlite::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down where nothing is yet");

        Self {
            fixture,
            store: SqliteThrottles::new(pool),
        }
    }

    fn store(&self) -> &Self::Store {
        &self.store
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

crate::conformance_tests!(OnSqlite);
