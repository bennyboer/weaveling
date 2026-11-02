use async_trait::async_trait;
use sqlx::PgPool;
use test_harness::PostgresFixture;

use crate::suite::Workbench;
use crate::{Failure, InMemoryTransactions, PostgresTransactions, Transaction, Transactions};
use crate::{memory, postgres};

const LAY_DOWN: &str = "CREATE TABLE words (nth BIGSERIAL PRIMARY KEY, word TEXT NOT NULL)";
const WRITE: &str = "INSERT INTO words (word) VALUES ($1)";
const WRITTEN: &str = "SELECT word FROM words ORDER BY nth";

struct OnPostgres {
    fixture: PostgresFixture,
    pool: PgPool,
    transactions: PostgresTransactions,
}

#[async_trait]
impl Workbench for OnPostgres {
    type Transactions = PostgresTransactions;

    async fn setup() -> Self {
        let fixture = PostgresFixture::setup().await;
        let pool: PgPool = fixture.create_schema("transacting").await;
        sqlx::query(LAY_DOWN)
            .execute(&pool)
            .await
            .expect("the table should lay down in an empty namespace");

        Self {
            fixture,
            transactions: PostgresTransactions::new(pool.clone()),
            pool,
        }
    }

    fn transactions(&self) -> &Self::Transactions {
        &self.transactions
    }

    async fn write(&self, transaction: &mut Transaction, word: &str) {
        sqlx::query(WRITE)
            .bind(word)
            .execute(
                postgres::connection(transaction)
                    .expect("a transaction begun on PostgreSQL has its connection"),
            )
            .await
            .expect("writing should succeed");
    }

    async fn written(&self) -> Vec<String> {
        sqlx::query_scalar(WRITTEN)
            .fetch_all(&self.pool)
            .await
            .expect("reading should succeed")
    }

    async fn cleanup(self) {
        self.fixture.cleanup().await;
    }
}

crate::conformance_tests!(OnPostgres);

#[tokio::test]
async fn a_transaction_begun_in_memory_has_no_postgres_connection() {
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");

    assert!(matches!(
        postgres::connection(&mut transaction),
        Err(Failure::BegunElsewhere)
    ));
}

#[tokio::test]
async fn a_transaction_begun_on_postgres_takes_no_undo() {
    let bench = OnPostgres::setup().await;
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");

    let undoing = memory::undo_on_rollback(&mut transaction, || {});

    assert!(matches!(undoing, Err(Failure::BegunElsewhere)));
    drop(transaction);
    bench.cleanup().await;
}

#[tokio::test]
async fn one_at_a_time_holds_a_second_transaction_on_the_key_until_the_first_is_over() {
    let bench = OnPostgres::setup().await;
    let mut first = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    postgres::one_at_a_time(&mut first, "appearance:idea:idea_1")
        .await
        .expect("the first takes the key at once");

    let same = tokio::spawn({
        let transactions = bench.transactions().clone();
        async move {
            let mut second = transactions
                .begin()
                .await
                .expect("beginning should succeed");
            postgres::one_at_a_time(&mut second, "appearance:idea:idea_1")
                .await
                .expect("the second takes the key once it is free");
            second.finish().await.expect("finishing should succeed");
        }
    });
    let mut other = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    tokio::time::timeout(
        std::time::Duration::from_secs(1),
        postgres::one_at_a_time(&mut other, "appearance:idea:idea_2"),
    )
    .await
    .expect("another key is not held")
    .expect("taking another key should succeed");
    other.finish().await.expect("finishing should succeed");
    tokio::time::sleep(std::time::Duration::from_millis(100)).await;
    let waited = !same.is_finished();
    first.finish().await.expect("finishing should succeed");

    assert!(
        waited,
        "the same key waits while the first transaction holds it"
    );
    tokio::time::timeout(std::time::Duration::from_secs(2), same)
        .await
        .expect("the second goes on once the first has committed")
        .expect("the second should not panic");
    bench.cleanup().await;
}
