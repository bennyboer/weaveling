use std::any::Any;

use async_trait::async_trait;
use sqlx::{PgConnection, PgPool, Postgres};

use crate::{Failure, Transaction, Transactions, Unit};

const ONE_AT_A_TIME: &str = "SELECT pg_advisory_xact_lock(hashtextextended($1, 0))";

#[derive(Clone)]
pub struct PostgresTransactions {
    pool: PgPool,
}

impl PostgresTransactions {
    pub fn new(pool: PgPool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Transactions for PostgresTransactions {
    async fn begin(&self) -> Result<Transaction, Failure> {
        let begun = self
            .pool
            .begin()
            .await
            .map_err(|failure| Failure::Backend(Box::new(failure)))?;

        Ok(Transaction::of(PostgresUnit(begun)))
    }
}

struct PostgresUnit(sqlx::Transaction<'static, Postgres>);

#[async_trait]
impl Unit for PostgresUnit {
    async fn commit(self: Box<Self>) -> Result<(), Failure> {
        self.0
            .commit()
            .await
            .map_err(|failure| Failure::Backend(Box::new(failure)))
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

pub fn connection(transaction: &mut Transaction) -> Result<&mut PgConnection, Failure> {
    Ok(&mut *transaction.backend::<PostgresUnit>()?.0)
}

/// Lets one transaction at a time past this point for `key`, until it commits or rolls back.
///
/// Two writes that each read before they write, as an upsert that checks for a tombstone and the forgetting that lays one, can both read before either commits under PostgreSQL's READ COMMITTED, and each miss the other. Taking the same key first, the second waits for the first and then reads what it committed. SQLite's one writer and the memory's one lock do this already, so only PostgreSQL adapters ask. The key is shared by the whole database, so it names what it holds, as `appearance:idea:idea_1` or an id that says its kind.
pub async fn one_at_a_time(transaction: &mut Transaction, key: &str) -> Result<(), Failure> {
    sqlx::query(ONE_AT_A_TIME)
        .bind(key)
        .execute(connection(transaction)?)
        .await
        .map_err(|failure| Failure::Backend(Box::new(failure)))?;

    Ok(())
}
