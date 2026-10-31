use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use async_trait::async_trait;
use transacting::{Transaction, memory};

use crate::{Registry, RegistryError};

#[derive(Debug, Default)]
pub struct InMemoryRegistry {
    claimed: Arc<Mutex<HashMap<(String, String), String>>>,
}

impl InMemoryRegistry {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Registry for InMemoryRegistry {
    async fn claim(
        &self,
        transaction: &mut Transaction,
        kind: &str,
        key: &str,
        id: &str,
    ) -> Result<String, RegistryError> {
        let key = (kind.to_owned(), key.to_owned());
        let mut claimed = self.claimed.lock().expect("registry lock poisoned");
        if let Some(held) = claimed.get(&key) {
            return Ok(held.clone());
        }

        let undone = key.clone();
        let registry = self.claimed.clone();
        memory::undo_on_rollback(transaction, move || {
            registry
                .lock()
                .expect("registry lock poisoned")
                .remove(&undone);
        })
        .map_err(|failure| RegistryError::Backend(Box::new(failure)))?;
        claimed.insert(key, id.to_owned());

        Ok(id.to_owned())
    }

    async fn holder(&self, kind: &str, key: &str) -> Result<Option<String>, RegistryError> {
        let claimed = self.claimed.lock().expect("registry lock poisoned");

        Ok(claimed.get(&(kind.to_owned(), key.to_owned())).cloned())
    }
}

#[cfg(test)]
mod tests {
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;

    use crate::suite::Workbench;

    struct InMemory(InMemoryRegistry, InMemoryTransactions);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryRegistry;

        async fn setup() -> Self {
            Self(InMemoryRegistry::new(), InMemoryTransactions::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.1
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
