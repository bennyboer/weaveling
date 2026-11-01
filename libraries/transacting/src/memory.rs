use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::{Failure, Transaction, Transactions, Unit};

type Undo = Box<dyn FnOnce() + Send>;

#[derive(Clone, Default)]
pub struct InMemoryTransactions {
    writing: Arc<Mutex<()>>,
    refusing_commits: bool,
}

impl InMemoryTransactions {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn refusing_commits() -> Self {
        Self {
            refusing_commits: true,
            ..Self::default()
        }
    }
}

#[async_trait]
impl Transactions for InMemoryTransactions {
    async fn begin(&self) -> Result<Transaction, Failure> {
        Ok(Transaction::of(InMemoryUnit {
            _writing: self.writing.clone().lock_owned().await,
            undos: Vec::new(),
            refusing_commits: self.refusing_commits,
        }))
    }
}

struct InMemoryUnit {
    _writing: OwnedMutexGuard<()>,
    undos: Vec<Undo>,
    refusing_commits: bool,
}

#[async_trait]
impl Unit for InMemoryUnit {
    async fn commit(mut self: Box<Self>) -> Result<(), Failure> {
        if self.refusing_commits {
            return Err(Failure::Backend("the commit was refused".into()));
        }
        self.undos.clear();

        Ok(())
    }

    fn as_any(&mut self) -> &mut dyn Any {
        self
    }
}

impl Drop for InMemoryUnit {
    fn drop(&mut self) {
        while let Some(undo) = self.undos.pop() {
            undo();
        }
    }
}

pub fn undo_on_rollback(
    transaction: &mut Transaction,
    undo: impl FnOnce() + Send + 'static,
) -> Result<(), Failure> {
    transaction
        .backend::<InMemoryUnit>()?
        .undos
        .push(Box::new(undo));

    Ok(())
}
