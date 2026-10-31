use std::any::Any;

use async_trait::async_trait;
use sqlx::{PgConnection, PgPool, Postgres};

use crate::{Failure, Transaction, Transactions, Unit};

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
