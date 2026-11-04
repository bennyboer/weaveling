use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;

use transacting::{PostgresTransactions, Transactions, Tried};

use crate::Registry;
use crate::postgres;
use crate::postgres::PostgresRegistry;
use crate::suite::Workbench;

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    store: PostgresRegistry,
    transactions: PostgresTransactions,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Store = PostgresRegistry;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("registry").await;
        postgres::migrations()
            .run(&pool)
            .await
            .expect("the schema should lay down in an empty namespace");

        Self {
            fixture,
            store: PostgresRegistry::new(pool.clone()),
            transactions: PostgresTransactions::new(pool.clone()),
            pool,
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
async fn a_race_for_one_key_leaves_exactly_one_winner() {
    // given: a registry
    let bench = OnPostgres::setup().await;
    let registry = std::sync::Arc::new(PostgresRegistry::new(bench.pool.clone()));

    // when: eight claim one key at once
    let racing: Vec<_> = (1..=8)
        .map(|nth| {
            let registry = registry.clone();
            let transactions = PostgresTransactions::new(bench.pool.clone());

            tokio::spawn(async move {
                transacting::retrying!(&transactions, |transaction| {
                    registry
                        .claim(transaction, "board", "project_1", &format!("board_{nth}"))
                        .await
                        .map_err(Tried::Refused)
                })
                .expect("claiming should succeed")
            })
        })
        .collect();

    let mut held = Vec::new();
    for attempt in racing {
        held.push(attempt.await.expect("the claim should not panic"));
    }

    // then: all are told the same holder
    let first = held.first().expect("eight claims were made").clone();
    assert!(
        held.iter().all(|held| held == &first),
        "eight concurrent opens must all be told the same board, got {held:?}"
    );

    bench.cleanup().await;
}
