use std::any::Any;
use std::fmt::{self, Debug, Formatter};
use std::future::Future;

use async_trait::async_trait;
use futures_util::future::BoxFuture;
use futures_util::stream::BoxStream;
use futures_util::{FutureExt, StreamExt, TryFutureExt, TryStreamExt};

use sqlx::postgres::{PgQueryResult, PgRow, PgStatement, PgTypeInfo};
use sqlx::{Describe, Either, Execute, Executor, PgPool, Postgres, SqlStr};

use crate::{Failure, Transaction, Transactions, Unit};

/// Runs on every connection of a feature's pool, so each of its transactions is serializable.
///
/// Two transactions that each read what the other writes cannot both commit.
/// The one that loses gets a serialization failure.
/// The transaction's `Connection` marks that failure, and `retrying!` tries again.
/// A transaction at another isolation level is not watched for that.
/// That is why the whole pool is set, not only `begin`.
pub const SERIALIZABLE: &str =
    "SET SESSION CHARACTERISTICS AS TRANSACTION ISOLATION LEVEL SERIALIZABLE";

const SERIALIZATION_FAILURE: &str = "40001";
const DEADLOCK_DETECTED: &str = "40P01";

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

        Ok(Transaction::of(PostgresUnit(Connection {
            begun,
            lost_race: false,
        })))
    }
}

struct PostgresUnit(Connection);

#[async_trait]
impl Unit for PostgresUnit {
    async fn commit(self: Box<Self>) -> Result<(), Failure> {
        self.0.begun.commit().await.map_err(failed)
    }

    fn lost_race(&self) -> bool {
        self.0.lost_race
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

/// A transaction's connection, as adapters reach it through `connection`.
///
/// An adapter may turn a failed query into a message in its own error.
/// Even then, the transaction remembers when a statement lost to another transaction, through a serialization failure or a deadlock.
/// So `retrying!` can retry the whole work, however the failure reached it.
pub struct Connection {
    begun: sqlx::Transaction<'static, Postgres>,
    lost_race: bool,
}

impl Debug for Connection {
    fn fmt(&self, formatter: &mut Formatter<'_>) -> fmt::Result {
        formatter
            .debug_struct("Connection")
            .field("lost_race", &self.lost_race)
            .finish_non_exhaustive()
    }
}

impl<'c> Executor<'c> for &'c mut Connection {
    type Database = Postgres;

    fn fetch_many<'e, 'q: 'e, E>(
        self,
        query: E,
    ) -> BoxStream<'e, Result<Either<PgQueryResult, PgRow>, sqlx::Error>>
    where
        'c: 'e,
        E: 'q + Execute<'q, Postgres>,
    {
        let Connection { begun, lost_race } = self;

        (&mut **begun)
            .fetch_many(query)
            .inspect_err(move |failure| *lost_race |= crate::postgres::lost(failure))
            .boxed()
    }

    fn fetch_optional<'e, 'q: 'e, E>(
        self,
        query: E,
    ) -> BoxFuture<'e, Result<Option<PgRow>, sqlx::Error>>
    where
        'c: 'e,
        E: 'q + Execute<'q, Postgres>,
    {
        let Connection { begun, lost_race } = self;

        (&mut **begun)
            .fetch_optional(query)
            .inspect_err(move |failure| *lost_race |= crate::postgres::lost(failure))
            .boxed()
    }

    fn prepare_with<'e>(
        self,
        sql: SqlStr,
        parameters: &'e [PgTypeInfo],
    ) -> BoxFuture<'e, Result<PgStatement, sqlx::Error>>
    where
        'c: 'e,
    {
        (&mut *self.begun).prepare_with(sql, parameters)
    }

    fn describe<'e>(self, sql: SqlStr) -> BoxFuture<'e, Result<Describe<Postgres>, sqlx::Error>>
    where
        'c: 'e,
    {
        (&mut *self.begun).describe(sql)
    }
}

pub fn connection(transaction: &mut Transaction) -> Result<&mut Connection, Failure> {
    Ok(&mut transaction.backend::<PostgresUnit>()?.0)
}

/// Runs a statement on the pool, outside any `Transaction`, and runs it again if it lost a race.
/// This works like `retrying!` does for a transaction.
/// A statement on its own is its own transaction, serializable like every other, so it can lose a race too.
pub async fn retried<T, F, Running>(mut statement: F) -> Result<T, sqlx::Error>
where
    F: FnMut() -> Running,
    Running: Future<Output = Result<T, sqlx::Error>>,
{
    let mut tried = 1;
    loop {
        match statement().await {
            Err(failure) if crate::postgres::lost(&failure) && tried < crate::retrying::TRIES => {
                crate::retrying::pause(tried).await;
                tried += 1;
            }
            outcome => return outcome,
        }
    }
}

fn failed(failure: sqlx::Error) -> Failure {
    match crate::postgres::lost(&failure) {
        true => Failure::LostRace,
        false => Failure::Backend(Box::new(failure)),
    }
}

fn lost(failure: &sqlx::Error) -> bool {
    failure
        .as_database_error()
        .and_then(|database| database.code())
        .is_some_and(|code| code == SERIALIZATION_FAILURE || code == DEADLOCK_DETECTED)
}
