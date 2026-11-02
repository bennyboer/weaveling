use std::any::Any;
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::{Arc, LazyLock, Mutex};

use async_trait::async_trait;
use sqlx::{Sqlite, SqliteConnection, SqlitePool};
use tokio::sync::{Mutex as Queue, OwnedMutexGuard};

use crate::{Failure, Transaction, Transactions, Unit};

const BEGIN_WRITING: &str = "BEGIN IMMEDIATE";

static WRITERS: LazyLock<Mutex<HashMap<PathBuf, Arc<Queue<()>>>>> =
    LazyLock::new(|| Mutex::new(HashMap::new()));

#[derive(Clone)]
pub struct SqliteTransactions {
    pool: SqlitePool,
    writing: Arc<Queue<()>>,
}

impl SqliteTransactions {
    pub fn new(pool: SqlitePool) -> Self {
        let file = pool.connect_options().get_filename().to_path_buf();
        let writing = WRITERS
            .lock()
            .expect("nothing panics holding the writers")
            .entry(file)
            .or_default()
            .clone();

        Self { pool, writing }
    }
}

#[async_trait]
impl Transactions for SqliteTransactions {
    async fn begin(&self) -> Result<Transaction, Failure> {
        let turn = self.writing.clone().lock_owned().await;
        let begun = self
            .pool
            .begin_with(BEGIN_WRITING)
            .await
            .map_err(|failure| Failure::Backend(Box::new(failure)))?;

        Ok(Transaction::of(SqliteUnit { begun, _turn: turn }))
    }
}

struct SqliteUnit {
    begun: sqlx::Transaction<'static, Sqlite>,
    _turn: OwnedMutexGuard<()>,
}

#[async_trait]
impl Unit for SqliteUnit {
    async fn commit(self: Box<Self>) -> Result<(), Failure> {
        self.begun
            .commit()
            .await
            .map_err(|failure| Failure::Backend(Box::new(failure)))
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

pub fn connection(transaction: &mut Transaction) -> Result<&mut SqliteConnection, Failure> {
    Ok(&mut *transaction.backend::<SqliteUnit>()?.begun)
}
