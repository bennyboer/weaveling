use accounts_core::{AccountId, Dismissal, Operators};
use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;
use time::{Duration, OffsetDateTime};
use transacting::{PostgresTransactions, Transactions};

use crate::postgres;
use crate::postgres::PostgresSecrets;
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    store: PostgresSecrets,
    transactions: PostgresTransactions,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresSecrets;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("accounts").await;
        postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            store: PostgresSecrets::new(pool.clone()),
            transactions: PostgresTransactions::new(pool),
        }
    }

    fn store(&self) -> &Self::Store {
        &self.store
    }

    fn transactions(&self) -> &dyn Transactions {
        &self.transactions
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

crate::conformance_tests!(OnPostgres);

#[tokio::test]
async fn a_dismissal_waits_for_an_appointment_under_way() {
    let bench = OnPostgres::setup().await;
    let (kept, newcomer) = (
        AccountId::generate(OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_000)),
        AccountId::generate(OffsetDateTime::UNIX_EPOCH + Duration::seconds(2_000)),
    );
    let mut appointing = bench
        .transactions
        .begin()
        .await
        .expect("beginning should succeed");
    bench
        .store
        .appoint(&mut appointing, &kept)
        .await
        .expect("appointing should succeed");
    appointing
        .finish()
        .await
        .expect("committing should succeed");
    let mut appointing = bench
        .transactions
        .begin()
        .await
        .expect("beginning should succeed");
    bench
        .store
        .appoint(&mut appointing, &newcomer)
        .await
        .expect("appointing should succeed");

    let (dismissal, ()) = tokio::join!(
        async {
            let mut dismissing = bench
                .transactions
                .begin()
                .await
                .expect("beginning should succeed");
            let dismissal = bench
                .store
                .dismiss(&mut dismissing, &newcomer)
                .await
                .expect("dismissing should succeed");
            dismissing
                .finish()
                .await
                .expect("committing should succeed");
            dismissal
        },
        async {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            appointing
                .finish()
                .await
                .expect("committing should succeed");
        }
    );

    assert_eq!(
        dismissal,
        Dismissal::Dismissed,
        "a dismissal that did not wait would find nobody to dismiss, and the appointment landing after it \
         would leave an operator the events never made"
    );
    bench.cleanup().await;
}

#[tokio::test]
async fn whether_one_is_an_operator_waits_for_a_dismissal_under_way() {
    let bench = OnPostgres::setup().await;
    let (kept, leaving) = (
        AccountId::generate(OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_000)),
        AccountId::generate(OffsetDateTime::UNIX_EPOCH + Duration::seconds(2_000)),
    );
    for account in [&kept, &leaving] {
        let mut appointing = bench
            .transactions
            .begin()
            .await
            .expect("beginning should succeed");
        bench
            .store
            .appoint(&mut appointing, account)
            .await
            .expect("appointing should succeed");
        appointing
            .finish()
            .await
            .expect("committing should succeed");
    }
    let mut dismissing = bench
        .transactions
        .begin()
        .await
        .expect("beginning should succeed");
    bench
        .store
        .dismiss(&mut dismissing, &leaving)
        .await
        .expect("dismissing should succeed");

    let (appointed, ()) = tokio::join!(
        async {
            let mut acting = bench
                .transactions
                .begin()
                .await
                .expect("beginning should succeed");
            let appointed = bench
                .store
                .is_appointed(&mut acting, &leaving)
                .await
                .expect("asking should succeed");
            acting.finish().await.expect("committing should succeed");
            appointed
        },
        async {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            dismissing
                .finish()
                .await
                .expect("committing should succeed");
        }
    );

    assert!(
        !appointed,
        "an operator acting while they are dismissed waits for the dismissal, or their act lands after it"
    );
    bench.cleanup().await;
}
