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
