pub mod memory;
#[cfg(test)]
mod memory_tests;
pub mod retrying;
#[cfg(test)]
mod retrying_tests;
#[cfg(test)]
mod suite;

#[cfg(feature = "postgres")]
pub mod postgres;
#[cfg(all(test, feature = "postgres"))]
mod postgres_tests;

#[cfg(feature = "sqlite")]
pub mod sqlite;
#[cfg(all(test, feature = "sqlite"))]
mod sqlite_tests;

pub use memory::InMemoryTransactions;
pub use retrying::LostRace;

#[cfg(feature = "postgres")]
pub use postgres::PostgresTransactions;

#[cfg(feature = "sqlite")]
pub use sqlite::SqliteTransactions;

use std::any::Any;
use std::error::Error;
use std::future::Future;
use std::ops::{Deref, DerefMut};
use std::pin::Pin;

use async_trait::async_trait;
use thiserror::Error;

#[derive(Debug, Error)]
pub enum Failure {
    #[error("the transaction's backend failed")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
    #[error("the transaction was begun by another backend than the one it was handed to")]
    BegunElsewhere,
}

#[async_trait]
pub trait Transactions: Send + Sync {
    async fn begin(&self) -> Result<Transaction, Failure>;

    async fn join<'a>(&self, within: Option<&'a mut Transaction>) -> Result<Joined<'a>, Failure> {
        match within {
            Some(transaction) => Ok(Joined::Borrowed(transaction)),
            None => Ok(Joined::Own(self.begin().await?)),
        }
    }
}

#[async_trait]
pub(crate) trait Unit: Send + 'static {
    async fn commit(self: Box<Self>) -> Result<(), Failure>;

    fn as_any(&mut self) -> &mut dyn Any;
}

type Followup = Pin<Box<dyn Future<Output = ()> + Send>>;

#[must_use = "a transaction is rolled back unless it is finished"]
pub struct Transaction {
    unit: Box<dyn Unit>,
    after_commit: Vec<Followup>,
}

impl Transaction {
    pub(crate) fn of(unit: impl Unit) -> Self {
        Self {
            unit: Box::new(unit),
            after_commit: Vec::new(),
        }
    }

    pub fn after_commit(&mut self, followup: impl Future<Output = ()> + Send + 'static) {
        self.after_commit.push(Box::pin(followup));
    }

    pub async fn finish(self) -> Result<(), Failure> {
        self.unit.commit().await?;
        for followup in self.after_commit {
            followup.await;
        }

        Ok(())
    }

    pub(crate) fn backend<U: Unit>(&mut self) -> Result<&mut U, Failure> {
        self.unit
            .as_any()
            .downcast_mut::<U>()
            .ok_or(Failure::BegunElsewhere)
    }
}

#[must_use = "a transaction of its own is rolled back unless it is finished"]
pub enum Joined<'a> {
    Own(Transaction),
    Borrowed(&'a mut Transaction),
}

impl Joined<'_> {
    pub async fn finish(self) -> Result<(), Failure> {
        match self {
            Joined::Own(transaction) => transaction.finish().await,
            Joined::Borrowed(_) => Ok(()),
        }
    }
}

impl Deref for Joined<'_> {
    type Target = Transaction;

    fn deref(&self) -> &Transaction {
        match self {
            Joined::Own(transaction) => transaction,
            Joined::Borrowed(transaction) => transaction,
        }
    }
}

impl DerefMut for Joined<'_> {
    fn deref_mut(&mut self) -> &mut Transaction {
        match self {
            Joined::Own(transaction) => transaction,
            Joined::Borrowed(transaction) => transaction,
        }
    }
}
