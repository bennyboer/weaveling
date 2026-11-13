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

    /// Returns how many pieces are stored, across all assets.
    pub fn pieces(&self) -> usize {
        self.kept().len()
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

    async fn stored(
        &self,
        after: Option<&AssetId>,
        at_most: u64,
    ) -> Result<Vec<AssetId>, ContentsError> {
        let mut stored: Vec<AssetId> = self.kept().keys().map(|(asset, _)| *asset).collect();
        stored.sort_by_key(ToString::to_string);
        stored.dedup();

        Ok(stored
            .into_iter()
            .filter(|asset| after.is_none_or(|last| asset.to_string() > last.to_string()))
            .take(usize::try_from(at_most).unwrap_or(usize::MAX))
            .collect())
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
