use async_trait::async_trait;
use sqlx::{AssertSqlSafe, PgPool};
use test_harness::PostgresFixture;

use crate::suite::Workbench;
use crate::{
    Failure, InMemoryTransactions, LostRace, PostgresTransactions, Transaction, Transactions,
};
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
    // given: a transaction begun in memory
    let mut transaction = InMemoryTransactions::new()
        .begin()
        .await
        .expect("beginning should succeed");

    // then: it has no postgres connection
    assert!(matches!(
        postgres::connection(&mut transaction),
        Err(Failure::BegunElsewhere)
    ));
}

#[tokio::test]
async fn a_transaction_begun_on_postgres_takes_no_undo() {
    // given: a transaction begun on postgres
    let bench = OnPostgres::setup().await;
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");

    // when: an undo is taken for it
    let undoing = memory::undo_on_rollback(&mut transaction, || {});

    // then: it is refused
    assert!(matches!(undoing, Err(Failure::BegunElsewhere)));
    drop(transaction);
    bench.cleanup().await;
}

#[derive(Debug, PartialEq)]
struct Refused(String);

impl LostRace for Refused {
    fn lost_race(&self) -> bool {
        false
    }
}

impl From<Failure> for Refused {
    fn from(failure: Failure) -> Self {
        Refused(failure.to_string())
    }
}

fn raising(code: &str) -> String {
    format!("DO $$ BEGIN RAISE EXCEPTION 'raised' USING ERRCODE = '{code}'; END $$")
}

async fn tries_until_it_raises_no_more(
    bench: &OnPostgres,
    code: &str,
) -> (Result<u8, Refused>, u8) {
    let mut tried = 0;
    let answered = crate::retrying!(bench.transactions(), |transaction| {
        tried += 1;
        if tried == 1 {
            sqlx::raw_sql(AssertSqlSafe(raising(code)))
                .execute(postgres::connection(transaction)?)
                .await
                .map_err(|failure| Refused(failure.to_string()))?;
        }
        Ok(tried)
    });

    (answered, tried)
}

#[tokio::test]
async fn a_statement_that_lost_a_race_is_tried_again_however_its_failure_was_answered() {
    // given: transactions on postgres
    let bench = OnPostgres::setup().await;

    // then: a statement that lost a race either way is tried once more and goes through
    for code in ["40001", "40P01"] {
        assert_eq!(
            tries_until_it_raises_no_more(&bench, code).await,
            (Ok(2), 2),
            "the adapter's own error says nothing of a race, but the transaction knows {code} lost one"
        );
    }
    bench.cleanup().await;
}

#[tokio::test]
async fn a_statement_failing_otherwise_is_not_tried_again() {
    // given: transactions on postgres
    let bench = OnPostgres::setup().await;

    // when: a statement fails other than by losing a race
    let (answered, tried) = tries_until_it_raises_no_more(&bench, "P0001").await;

    // then: it is not tried again
    assert!(answered.is_err());
    assert_eq!(tried, 1);
    bench.cleanup().await;
}

#[tokio::test]
async fn a_commit_that_lost_a_race_is_tried_again() {
    // given: transactions on postgres
    let bench = OnPostgres::setup().await;
    // and: a table whose rows lose a race when committed
    sqlx::raw_sql(AssertSqlSafe(
        "CREATE TABLE doomed (nth BIGINT);
         CREATE FUNCTION doom() RETURNS trigger AS $doom$ BEGIN RAISE EXCEPTION 'doomed' USING ERRCODE = '40001'; END $doom$ LANGUAGE plpgsql;
         CREATE CONSTRAINT TRIGGER doomed_at_commit AFTER INSERT ON doomed
             DEFERRABLE INITIALLY DEFERRED FOR EACH ROW EXECUTE FUNCTION doom();"
    ))
    .execute(&bench.pool)
    .await
    .expect("the doomed table should lay down");
    // when: a row is written to it
    let mut tried = 0;

    let answered: Result<u8, Refused> = crate::retrying!(bench.transactions(), |transaction| {
        tried += 1;
        if tried == 1 {
            sqlx::query("INSERT INTO doomed (nth) VALUES (1)")
                .execute(postgres::connection(transaction)?)
                .await
                .map_err(|failure| Refused(failure.to_string()))?;
        }
        Ok(tried)
    });

    // then: the write is tried once more and goes through
    assert_eq!(
        (answered, tried),
        (Ok(2), 2),
        "the race is often lost only when committing, the statements all gone through"
    );
    bench.cleanup().await;
}

#[tokio::test]
async fn a_row_fetched_that_lost_a_race_is_tried_again() {
    // given: transactions on postgres
    let bench = OnPostgres::setup().await;
    // and: a function that loses a race when called
    sqlx::raw_sql(AssertSqlSafe(
        "CREATE FUNCTION lose() RETURNS BIGINT AS $lose$ BEGIN RAISE EXCEPTION 'lost' USING ERRCODE = '40001'; END $lose$ LANGUAGE plpgsql;",
    ))
    .execute(&bench.pool)
    .await
    .expect("the losing function should lay down");
    // when: a row is fetched from it
    let mut tried = 0;

    let answered: Result<u8, Refused> = crate::retrying!(bench.transactions(), |transaction| {
        tried += 1;
        if tried == 1 {
            sqlx::query_scalar::<_, i64>("SELECT lose()")
                .fetch_optional(postgres::connection(transaction)?)
                .await
                .map_err(|failure| Refused(failure.to_string()))?;
        }
        Ok(tried)
    });

    // then: the fetch is tried once more and goes through
    assert_eq!((answered, tried), (Ok(2), 2));
    bench.cleanup().await;
}

#[tokio::test]
async fn a_write_given_one_try_is_still_tried_again_when_its_transaction_lost_a_race() {
    // given: transactions on postgres
    let bench = OnPostgres::setup().await;
    // when: a write given one try loses a race in the database
    let mut tried = 0;

    let answered: Result<u8, Refused> = crate::retrying!(bench.transactions(), 1, |transaction| {
        tried += 1;
        if tried == 1 {
            sqlx::raw_sql(AssertSqlSafe(raising("40001")))
                .execute(postgres::connection(transaction)?)
                .await
                .map_err(|failure| Refused(failure.to_string()))?;
        }
        Ok(tried)
    });

    // then: it is tried once more and goes through
    assert_eq!(
        (answered, tried),
        (Ok(2), 2),
        "one try keeps a write aimed at a version from papering over its moving on, which a race in the database is not"
    );
    bench.cleanup().await;
}

async fn writing_the_first_word(
    bench: &OnPostgres,
    word: &str,
    both_read: &tokio::sync::Barrier,
) -> u8 {
    let mut tried = 0;
    let answered: Result<(), Refused> = crate::retrying!(bench.transactions(), |transaction| {
        tried += 1;
        let written: i64 = sqlx::query_scalar("SELECT count(*) FROM words")
            .fetch_one(postgres::connection(transaction)?)
            .await
            .map_err(|failure| Refused(failure.to_string()))?;
        if tried == 1 {
            both_read.wait().await;
        }
        if written == 0 {
            sqlx::query(WRITE)
                .bind(word)
                .execute(postgres::connection(transaction)?)
                .await
                .map_err(|failure| Refused(failure.to_string()))?;
        }
        Ok(())
    });
    answered.expect("the race lost is tried again until it is won");

    tried
}

#[tokio::test]
async fn two_writers_that_each_read_nothing_written_yet_cannot_both_write() {
    // given: transactions on postgres with nothing written yet
    let bench = OnPostgres::setup().await;
    let both_read = tokio::sync::Barrier::new(2);

    // when: two writers each read nothing written and then write
    let (warp, weft) = tokio::join!(
        writing_the_first_word(&bench, "warp", &both_read),
        writing_the_first_word(&bench, "weft", &both_read)
    );

    // then: only one of them wrote
    assert_eq!(
        bench.written().await.len(),
        1,
        "each read the table empty, so one of them must have lost and read it again"
    );
    // and: one of them tried again
    assert!(
        warp.max(weft) >= 2,
        "the one that lost tried again, more than once where serializable gave up on both"
    );
    bench.cleanup().await;
}

#[tokio::test]
async fn a_transaction_on_a_feature_pool_is_serializable() {
    // given: a transaction begun on postgres
    let bench = OnPostgres::setup().await;
    let mut transaction = bench
        .transactions()
        .begin()
        .await
        .expect("beginning should succeed");

    // when: its isolation is asked
    let isolation: String = sqlx::query_scalar("SHOW transaction_isolation")
        .fetch_one(postgres::connection(&mut transaction).expect("a PostgreSQL transaction"))
        .await
        .expect("asking should succeed");

    // then: it is serializable
    assert_eq!(isolation, "serializable");
    drop(transaction);
    bench.cleanup().await;
}
