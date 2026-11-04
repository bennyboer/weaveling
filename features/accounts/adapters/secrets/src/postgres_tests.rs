use accounts_core::{AccountId, Dismissal, Operators};
use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;
use time::{Duration, OffsetDateTime};
use transacting::{PostgresTransactions, Transactions, Tried};

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
async fn a_dismissal_and_an_appointment_under_way_end_as_if_one_went_first() {
    // given: an appointed operator
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
    // and: a newcomer's appointment under way
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

    // when: the newcomer is dismissed while the appointment is finished
    let (dismissal, ()) = tokio::join!(
        async {
            transacting::retrying!(&bench.transactions, |dismissing| {
                bench
                    .store
                    .dismiss(dismissing, &newcomer)
                    .await
                    .map_err(Tried::Refused)
            })
            .expect("dismissing should succeed")
        },
        async {
            tokio::time::sleep(std::time::Duration::from_millis(300)).await;
            appointing
                .finish()
                .await
                .expect("committing should succeed");
        }
    );

    // then: the two end as if one went first
    let mut asking = bench
        .transactions
        .begin()
        .await
        .expect("beginning should succeed");
    let appointed = bench
        .store
        .is_appointed(&mut asking, &newcomer)
        .await
        .expect("asking should succeed");
    drop(asking);

    assert!(
        matches!(
            (dismissal, appointed),
            (Dismissal::Dismissed, false) | (Dismissal::NotAppointed, true)
        ),
        "the two end as if one went first, a dismissal told there was nobody to dismiss leaving the newcomer \
         appointed, as the events in that order do: {dismissal:?}, {appointed}"
    );
    bench.cleanup().await;
}
