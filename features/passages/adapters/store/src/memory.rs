use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::enqueuing::{Mapping, PassageMessageMapping};
use async_trait::async_trait;
use clock::Clock;
use eventsourcing::InMemoryOutbox;
use passages_core::{
    IdeaLink, Passage, PassageChange, PassageId, PassageStore, PassageTitle, ProjectLink,
    StoreError,
};

type Passages = HashMap<PassageId, StoredPassage>;

#[derive(Debug, Clone)]
struct StoredPassage {
    project: ProjectLink,
    title: PassageTitle,
    ideas: Vec<IdeaLink>,
    state: Vec<u8>,
}

#[derive(Default)]
pub struct InMemoryPassageStore {
    passages: RwLock<Passages>,
    enqueuing: Option<(Arc<InMemoryOutbox>, Mapping)>,
}

impl InMemoryPassageStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueuing_to(
        self,
        outbox: Arc<InMemoryOutbox>,
        clock: Arc<dyn Clock>,
        message_for: PassageMessageMapping,
    ) -> Self {
        Self {
            enqueuing: Some((outbox, Mapping::new(clock, message_for))),
            ..self
        }
    }

    fn enqueue_change(&self, change: PassageChange) {
        if let Some((outbox, mapping)) = &self.enqueuing {
            outbox.enqueue(mapping.message(&change));
        }
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
            self.enqueue_change(PassageChange::IdeaLinked {
                passage: id,
                idea: idea.clone(),
            });
        }

        Ok(())
    }

    async fn unlink(&self, id: PassageId, idea: &IdeaLink) -> Result<(), StoreError> {
        let mut passages = self.write();
        let stored = passages.get_mut(&id).ok_or(StoreError::NotFound(id))?;
        let before = stored.ideas.len();
        stored.ideas.retain(|held| held != idea);

        if stored.ideas.len() < before {
            self.enqueue_change(PassageChange::IdeaUnlinked {
                passage: id,
                idea: idea.clone(),
            });
        }

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
        let mut passages = self.write();
        passages.remove(&id).ok_or(StoreError::NotFound(id))?;
        self.enqueue_change(PassageChange::Deleted { passage: id });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use clock::FixedClock;
    use eventsourcing::Outbox;
    use messaging::Message;

    use super::*;
    use crate::suite::{Heard, Workbench, at, message_for};

    struct InMemory {
        store: InMemoryPassageStore,
        outbox: Arc<InMemoryOutbox>,
        heard: Arc<Heard>,
    }

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemoryPassageStore;

        async fn setup() -> Self {
            let clock = Arc::new(FixedClock::new(at(2_000)));
            let heard = Arc::new(Heard::default());
            let outbox = Arc::new(InMemoryOutbox::new(heard.clone(), clock.clone()));

            Self {
                store: InMemoryPassageStore::new().enqueuing_to(outbox.clone(), clock, message_for),
                outbox,
                heard,
            }
        }

        fn store(&self) -> &Self::Store {
            &self.store
        }

        async fn enqueued(&self) -> Vec<Message> {
            self.outbox
                .deliver(100)
                .await
                .expect("the in-memory outbox always delivers");

            self.heard.messages()
        }

        async fn cleanup(self) {}
    }

    crate::suite::conformance_tests!(InMemory);
}
