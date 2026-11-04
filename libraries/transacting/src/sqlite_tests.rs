use std::time::Duration;

use async_trait::async_trait;
use sqlx::SqlitePool;
use test_harness::SqliteFixture;

use crate::sqlite;
use crate::suite::Workbench;
use crate::{Failure, InMemoryTransactions, SqliteTransactions, Transaction, Transactions};

const LAY_DOWN: &str =
    "CREATE TABLE words (nth INTEGER PRIMARY KEY AUTOINCREMENT, word TEXT NOT NULL)";
const WRITE: &str = "INSERT INTO words (word) VALUES (?1)";
const WRITTEN: &str = "SELECT word FROM words ORDER BY nth";

const A_MOMENT: Duration = Duration::from_millis(50);

struct OnSqlite {
    fixture: SqliteFixture,
    pool: SqlitePool,
    transactions: SqliteTransactions,
}

#[async_trait]
impl Workbench for OnSqlite {
    type Transactions = SqliteTransactions;

    async fn setup() -> Self {
        let fixture = SqliteFixture::setup();
        let pool: SqlitePool = fixture.create_database("transacting").await;
        sqlx::query(LAY_DOWN)
            .execute(&pool)
            .await
            .expect("the table should lay down in an empty file");

        Self {
            fixture,
            transactions: SqliteTransactions::new(pool.clone()),
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
                sqlite::connection(transaction)
                    .expect("a transaction begun on SQLite has its connection"),
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

crate::conformance_tests!(OnSqlite);

#[tokio::test]
async fn a_second_transaction_begins_only_once_the_first_is_over() {
    // given: a transaction begun on sqlite
    let bench = OnSqlite::setup().await;
    let first = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");

    // when: a second is begun while it lasts
    let waiting = tokio::spawn({
        let transactions = bench.transactions().clone();
        async move {
            transactions
                .begin()
                .await
                .expect("beginning should succeed")
                .finish()
                .await
                .expect("finishing should succeed");
        }
    });
    // and: the first is finished a moment later
    tokio::time::sleep(A_MOMENT).await;
    let waited = !waiting.is_finished();
    first.finish().await.expect("finishing should succeed");

    // then: the second waited for it
    assert!(
        waited,
        "a writer takes the lock as it begins, so what it reads is still so when it writes"
    );
    // and: began once it was over
    tokio::time::timeout(A_MOMENT * 20, waiting)
        .await
        .expect("the second begins once the first is finished")
        .expect("the second should not panic");
    bench.cleanup().await;
}

#[tokio::test]
async fn a_transaction_begun_in_memory_has_no_sqlite_connection() {
    // given: a transaction begun in memory
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");

    // then: it has no sqlite connection
    assert!(matches!(
        sqlite::connection(&mut transaction),
        Err(Failure::BegunElsewhere)
    ));
}

#[tokio::test]
async fn writers_waiting_their_turn_leave_the_pool_to_the_one_writing() {
    // given: a transaction writing on sqlite
    let bench = OnSqlite::setup().await;
    let writing = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");
    // and: sixteen more writers waiting their turn
    let waiting: Vec<_> = (0..16)
        .map(|_| {
            let transactions = SqliteTransactions::new(bench.pool.clone());
            tokio::spawn(async move {
                transactions
                    .begin()
                    .await
                    .expect("beginning should succeed")
                    .finish()
                    .await
                    .expect("finishing should succeed");
            })
        })
        .collect();
    // when: the one writing reads and finishes
    tokio::time::sleep(A_MOMENT).await;
    let read = tokio::time::timeout(A_MOMENT * 20, bench.written()).await;
    writing.finish().await.expect("finishing should succeed");

    // then: it found a connection to read with
    assert!(
        read.is_ok(),
        "the one writing still finds a connection to read with, or it waits on those waiting on it"
    );
    // and: each waiting writer takes its turn
    for waiter in waiting {
        tokio::time::timeout(A_MOMENT * 40, waiter)
            .await
            .expect("each waiting writer takes its turn")
            .expect("a waiting writer should not panic");
    }
    bench.cleanup().await;
}
