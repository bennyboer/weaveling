use async_trait::async_trait;
use test_harness::PostgresFixture;

use crate::postgres;
use crate::postgres::PostgresInvitationLinks;
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    store: PostgresInvitationLinks,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresInvitationLinks;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool = fixture.create_schema("invitations").await;
        postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down where nothing is yet");

        Self {
            fixture,
            store: PostgresInvitationLinks::new(pool),
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
