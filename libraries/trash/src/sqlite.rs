use async_trait::async_trait;
use clock::text;
use sqlx::SqlitePool;
use sqlx::migrate::Migrator;
use time::OffsetDateTime;
use transacting::Transaction;

use crate::{Trash, TrashError, Trashed};

const LEDGER: &str = "_sqlx_migrations_trash";

const KEEP: &str = "
    INSERT INTO trash (kind, id, kept_until)
    VALUES (?1, ?2, ?3)
    ON CONFLICT (kind, id) DO UPDATE SET kept_until = excluded.kept_until
";

const TAKE_OUT: &str = "DELETE FROM trash WHERE kind = ?1 AND id = ?2";

const DUE: &str = "
    SELECT kind, id
    FROM trash
    WHERE kept_until <= ?1
    ORDER BY kept_until, kind, id
    LIMIT ?2
";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/sqlite");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct SqliteTrash {
    pool: SqlitePool,
}

impl SqliteTrash {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Trash for SqliteTrash {
    async fn put_into(
        &self,
        transaction: &mut Transaction,
        kind: &str,
        id: &str,
        until: OffsetDateTime,
    ) -> Result<(), TrashError> {
        sqlx::query(KEEP)
            .bind(kind)
            .bind(id)
            .bind(text::written(until))
            .execute(transacting::sqlite::connection(transaction)?)
            .await
            .map_err(unreachable)?;

        Ok(())
    }

    async fn take_out(
        &self,
        transaction: &mut Transaction,
        kind: &str,
        id: &str,
    ) -> Result<bool, TrashError> {
        let taken = sqlx::query(TAKE_OUT)
            .bind(kind)
            .bind(id)
            .execute(transacting::sqlite::connection(transaction)?)
            .await
            .map_err(unreachable)?;

        Ok(taken.rows_affected() > 0)
    }

    async fn due(&self, at: OffsetDateTime, at_most: u64) -> Result<Vec<Trashed>, TrashError> {
        let due: Vec<(String, String)> = sqlx::query_as(DUE)
            .bind(text::written(at))
            .bind(i64::try_from(at_most).unwrap_or(i64::MAX))
            .fetch_all(&self.pool)
            .await
            .map_err(unreachable)?;

        Ok(due
            .into_iter()
            .map(|(kind, id)| Trashed { kind, id })
            .collect())
    }
}

fn unreachable(failure: sqlx::Error) -> TrashError {
    TrashError::Unreachable(failure.to_string())
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;
    use test_harness::SqliteFixture;
    use transacting::{SqliteTransactions, Transactions};

    use super::SqliteTrash;
    use crate::suite::Workbench;

    struct OnSqlite {
        fixture: SqliteFixture,
        trash: SqliteTrash,
        transactions: SqliteTransactions,
    }

    #[async_trait]
    impl Workbench for OnSqlite {
        type Trash = SqliteTrash;

        async fn setup() -> Self {
            let fixture = SqliteFixture::setup();
            let pool = fixture.create_database("trash").await;
            super::migrations()
                .run(&pool)
                .await
                .expect("the trash should lay down in an empty file");

            Self {
                fixture,
                trash: SqliteTrash::new(pool.clone()),
                transactions: SqliteTransactions::new(pool),
            }
        }

        fn trash(&self) -> &SqliteTrash {
            &self.trash
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.transactions
        }

        async fn cleanup(self) {
            self.fixture.cleanup().await;
        }
    }

    crate::conformance_tests!(OnSqlite);
}
