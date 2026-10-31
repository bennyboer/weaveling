use std::any::Any;
use std::sync::Arc;

use async_trait::async_trait;
use tokio::sync::{Mutex, OwnedMutexGuard};

use crate::{Failure, Transaction, Transactions, Unit};

type Undo = Box<dyn FnOnce() + Send>;

#[derive(Clone, Default)]
pub struct InMemoryTransactions {
    writing: Arc<Mutex<()>>,
}

impl InMemoryTransactions {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Transactions for InMemoryTransactions {
    async fn begin(&self) -> Result<Transaction, Failure> {
        Ok(Transaction::of(InMemoryUnit {
            _writing: self.writing.clone().lock_owned().await,
            undos: Vec::new(),
        }))
    }
}

struct InMemoryUnit {
    _writing: OwnedMutexGuard<()>,
    undos: Vec<Undo>,
}

#[async_trait]
impl Unit for InMemoryUnit {
    async fn commit(mut self: Box<Self>) -> Result<(), Failure> {
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
