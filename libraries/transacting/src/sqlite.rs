use std::any::Any;

use async_trait::async_trait;
use sqlx::{Sqlite, SqliteConnection, SqlitePool};

use crate::{Failure, Transaction, Transactions, Unit};

const BEGIN_WRITING: &str = "BEGIN IMMEDIATE";

#[derive(Clone)]
pub struct SqliteTransactions {
    pool: SqlitePool,
}

impl SqliteTransactions {
    pub fn new(pool: SqlitePool) -> Self {
        Self { pool }
    }
}

#[async_trait]
impl Transactions for SqliteTransactions {
    async fn begin(&self) -> Result<Transaction, Failure> {
        let begun = self
            .pool
            .begin_with(BEGIN_WRITING)
            .await
            .map_err(|failure| Failure::Backend(Box::new(failure)))?;

        Ok(Transaction::of(SqliteUnit(begun)))
    }
}

struct SqliteUnit(sqlx::Transaction<'static, Sqlite>);

#[async_trait]
impl Unit for SqliteUnit {
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

pub fn connection(transaction: &mut Transaction) -> Result<&mut SqliteConnection, Failure> {
    Ok(&mut *transaction.backend::<SqliteUnit>()?.0)
}
