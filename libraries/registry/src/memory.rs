use std::collections::HashMap;
use std::sync::Mutex;

use async_trait::async_trait;

use crate::{Registry, RegistryError};

#[derive(Debug, Default)]
pub struct InMemoryRegistry {
    claimed: Mutex<HashMap<(String, String), String>>,
}

impl InMemoryRegistry {
    pub fn new() -> Self {
        Self::default()
    }
}

#[async_trait]
impl Registry for InMemoryRegistry {
    async fn claim(&self, kind: &str, key: &str, id: &str) -> Result<String, RegistryError> {
        let mut claimed = self.claimed.lock().expect("registry lock poisoned");

        Ok(claimed
            .entry((kind.to_owned(), key.to_owned()))
            .or_insert_with(|| id.to_owned())
            .clone())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    use crate::suite::Workbench;

    struct InMemory(InMemoryRegistry);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryRegistry;

        async fn setup() -> Self {
            Self(InMemoryRegistry::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
