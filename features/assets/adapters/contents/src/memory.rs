use std::collections::HashMap;
use std::sync::{Mutex, MutexGuard};

use assets_core::{AssetId, Contents, ContentsError, Piece};
use async_trait::async_trait;

#[derive(Debug, Default)]
pub struct InMemoryContents {
    kept: Mutex<HashMap<(AssetId, Piece), Vec<u8>>>,
}

impl InMemoryContents {
    pub fn new() -> Self {
        Self::default()
    }

    fn kept(&self) -> MutexGuard<'_, HashMap<(AssetId, Piece), Vec<u8>>> {
        self.kept.lock().expect("the contents are not poisoned")
    }
}

#[async_trait]
impl Contents for InMemoryContents {
    async fn keep(&self, asset: &AssetId, piece: Piece, bytes: &[u8]) -> Result<(), ContentsError> {
        self.kept().insert((*asset, piece), bytes.to_vec());

        Ok(())
    }

    async fn read(&self, asset: &AssetId, piece: Piece) -> Result<Option<Vec<u8>>, ContentsError> {
        Ok(self.kept().get(&(*asset, piece)).cloned())
    }

    async fn forget(&self, asset: &AssetId) -> Result<(), ContentsError> {
        self.kept().retain(|(held, _), _| held != asset);

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use async_trait::async_trait;

    use super::*;
    use crate::suite::Workbench;

    struct InMemory(InMemoryContents);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryContents;

        async fn setup() -> Self {
            Self(InMemoryContents::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        async fn cleanup(self) {}
    }

    crate::conformance_tests!(InMemory);
}
