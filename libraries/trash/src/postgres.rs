use async_trait::async_trait;
use sqlx::PgPool;
use sqlx::migrate::Migrator;
use time::OffsetDateTime;
use transacting::Transaction;

use crate::{Trash, TrashError, Trashed};

const LEDGER: &str = "_sqlx_migrations_trash";

const KEEP: &str = "
    INSERT INTO trash (kind, id, kept_until)
    VALUES ($1, $2, $3)
    ON CONFLICT (kind, id) DO UPDATE SET kept_until = excluded.kept_until
";

const TAKE_OUT: &str = "DELETE FROM trash WHERE kind = $1 AND id = $2";

const DUE: &str = "
    SELECT kind, id
    FROM trash
    WHERE kept_until <= $1
    ORDER BY kept_until, kind, id
    LIMIT $2
";

pub fn migrations() -> Migrator {
    let mut laying = sqlx::migrate!("./migrations/postgres");
    laying.dangerous_set_table_name(LEDGER);

    laying
}

pub struct PostgresTrash {
    pool: PgPool,
}

impl PostgresTrash {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Trash for PostgresTrash {
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
            .bind(until)
            .execute(transacting::postgres::connection(transaction)?)
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
            .execute(transacting::postgres::connection(transaction)?)
            .await
            .map_err(unreachable)?;

        Ok(taken.rows_affected() > 0)
    }

    async fn due(&self, at: OffsetDateTime, at_most: u64) -> Result<Vec<Trashed>, TrashError> {
        let due: Vec<(String, String)> = sqlx::query_as(DUE)
            .bind(at)
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
    use test_harness::PostgresFixture;
    use transacting::{PostgresTransactions, Transactions};

    use super::PostgresTrash;
    use crate::suite::Workbench;

    struct OnPostgres {
        fixture: PostgresFixture,
        trash: PostgresTrash,
        transactions: PostgresTransactions,
    }

    #[async_trait]
    impl Workbench for OnPostgres {
        type Trash = PostgresTrash;

        async fn setup() -> Self {
            let fixture = PostgresFixture::setup().await;
            let pool = fixture.create_schema("trash").await;
            super::migrations()
                .run(&pool)
                .await
                .expect("the trash should lay down in an empty namespace");

            Self {
                fixture,
                trash: PostgresTrash::new(pool.clone()),
                transactions: PostgresTransactions::new(pool),
            }
        }

        fn trash(&self) -> &PostgresTrash {
            &self.trash
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.transactions
        }

        async fn cleanup(self) {
            self.fixture.cleanup().await;
        }
    }

    crate::conformance_tests!(OnPostgres);
}
