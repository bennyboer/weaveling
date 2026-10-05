use std::collections::HashMap;
use std::sync::{RwLock, RwLockReadGuard, RwLockWriteGuard};

use async_trait::async_trait;
use passages_core::{
    IdeaLink, Passage, PassageId, PassageStore, PassageTitle, ProjectLink, StoreError,
};

type Passages = HashMap<PassageId, StoredPassage>;

#[derive(Debug, Clone)]
struct StoredPassage {
    project: ProjectLink,
    title: PassageTitle,
    ideas: Vec<IdeaLink>,
    state: Vec<u8>,
}

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

fn rehydrate(id: PassageId, stored: &StoredPassage) -> Result<Passage, StoreError> {
    Passage::rehydrate(id, stored.project.clone(), &stored.state)
        .map(|passage| {
            passage
                .titled(stored.title.clone())
                .linked_to(stored.ideas.clone())
        })
        .map_err(|reason| StoreError::Backend(Box::new(reason)))
}

#[async_trait]
impl PassageStore for InMemoryPassageStore {
    async fn create(&self, passage: &Passage) -> Result<(), StoreError> {
        let mut passages = self.write();

        if passages.contains_key(&passage.id()) {
            return Err(StoreError::Conflict(passage.id()));
        }

        passages.insert(
            passage.id(),
            StoredPassage {
                project: passage.project().clone(),
                title: passage.title().clone(),
                ideas: passage.ideas().to_vec(),
                state: passage.everything(),
            },
        );

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

    async fn apply(&self, id: PassageId, update: &[u8]) -> Result<(), StoreError> {
        let mut passages = self.write();

        let stored = passages.get(&id).cloned().ok_or(StoreError::NotFound(id))?;
        let passage = rehydrate(id, &stored)?;
        passage
            .apply(update)
            .map_err(|_| StoreError::Unusable(id))?;

        passages.insert(
            id,
            StoredPassage {
                project: stored.project,
                title: stored.title,
                ideas: stored.ideas,
                state: passage.everything(),
            },
        );

        Ok(())
    }

    async fn retitle(&self, id: PassageId, title: &PassageTitle) -> Result<(), StoreError> {
        let mut passages = self.write();
        let stored = passages.get_mut(&id).ok_or(StoreError::NotFound(id))?;
        stored.title = title.clone();

        Ok(())
    }

    async fn link(&self, id: PassageId, idea: &IdeaLink) -> Result<(), StoreError> {
        let mut passages = self.write();
        let stored = passages.get_mut(&id).ok_or(StoreError::NotFound(id))?;

        if !stored.ideas.contains(idea) {
            stored.ideas.push(idea.clone());
        }

        Ok(())
    }

    async fn unlink(&self, id: PassageId, idea: &IdeaLink) -> Result<(), StoreError> {
        let mut passages = self.write();
        let stored = passages.get_mut(&id).ok_or(StoreError::NotFound(id))?;
        stored.ideas.retain(|held| held != idea);

        Ok(())
    }

    async fn unlink_everywhere(&self, idea: &IdeaLink) -> Result<(), StoreError> {
        for stored in self.write().values_mut() {
            stored.ideas.retain(|held| held != idea);
        }

        Ok(())
    }

    async fn in_project(
        &self,
        project: &ProjectLink,
        after: Option<PassageId>,
        at_most: usize,
    ) -> Result<Vec<PassageId>, StoreError> {
        let mut found: Vec<PassageId> = self
            .read()
            .iter()
            .filter(|(_, stored)| &stored.project == project)
            .map(|(id, _)| *id)
            .filter(|id| after.is_none_or(|last| id > &last))
            .collect();
        found.sort();
        found.truncate(at_most);

        Ok(found)
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
    use crate::suite::Workbench;

    struct InMemory(InMemoryPassageStore);

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryPassageStore;

        async fn setup() -> Self {
            Self(InMemoryPassageStore::new())
        }

        fn store(&self) -> &Self::Store {
            &self.0
        }

        async fn cleanup(self) {}
    }

    crate::suite::conformance_tests!(InMemory);
}
