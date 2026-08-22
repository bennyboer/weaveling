use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use passages_core::{Passage, PassageId, PassageStore, StoreError};

type Passages = HashMap<PassageId, Vec<u8>>;

#[derive(Debug, Default)]
pub struct InMemoryPassageStore {
    passages: RwLock<Passages>,
}

impl InMemoryPassageStore {
    pub fn new() -> Self {
        Self::default()
    }

    fn read(&self) -> RwLockReadGuard<'_, Passages> {
        self.passages.read().expect("passage store lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Passages> {
        self.passages.write().expect("passage store lock poisoned")
    }
}

fn rehydrate(id: PassageId, stored: &[u8]) -> Result<Passage, StoreError> {
    Passage::rehydrate(id, stored).map_err(|reason| StoreError::Backend(Box::new(reason)))
}

#[async_trait]
impl PassageStore for InMemoryPassageStore {
    async fn create(&self, passage: &Passage) -> Result<(), StoreError> {
        let mut passages = self.write();

        if passages.contains_key(&passage.id()) {
            return Err(StoreError::Conflict(passage.id()));
        }

        passages.insert(passage.id(), passage.everything());

        Ok(())
    }

    async fn load(&self, id: PassageId) -> Result<Passage, StoreError> {
        let stored = self
            .read()
            .get(&id)
            .cloned()
            .ok_or(StoreError::NotFound(id))?;

        rehydrate(id, &stored)
    }

    async fn absorb(&self, id: PassageId, update: &[u8]) -> Result<(), StoreError> {
        let mut passages = self.write();

        let stored = passages.get(&id).cloned().ok_or(StoreError::NotFound(id))?;
        let passage = rehydrate(id, &stored)?;
        passage
            .absorb(update)
            .map_err(|_| StoreError::Unusable(id))?;

        passages.insert(id, passage.everything());

        Ok(())
    }

    async fn delete(&self, id: PassageId) -> Result<(), StoreError> {
        self.write()
            .remove(&id)
            .map(|_| ())
            .ok_or(StoreError::NotFound(id))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    crate::suite::conformance_tests!(InMemoryPassageStore::new());
}
