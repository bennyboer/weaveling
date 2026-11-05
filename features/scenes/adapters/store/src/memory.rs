use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::enqueuing::{Mapping, SceneMessageMapping};
use async_trait::async_trait;
use clock::Clock;
use outbox::InMemoryOutbox;
use scenes_core::{
    IdeaLink, ProjectLink, Scene, SceneChange, SceneId, SceneStore, SceneTitle, StoreError,
};
use transacting::Transaction;

type Scenes = HashMap<SceneId, StoredScene>;

#[derive(Debug, Clone)]
struct StoredScene {
    project: ProjectLink,
    title: SceneTitle,
    ideas: Vec<IdeaLink>,
    version: u64,
    prose_version: u64,
    state: Vec<u8>,
    uncounted: bool,
}

#[derive(Default)]
pub struct InMemorySceneStore {
    scenes: Arc<RwLock<Scenes>>,
    enqueuing: Option<(Arc<InMemoryOutbox>, Mapping)>,
}

impl InMemorySceneStore {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn enqueuing_to(
        self,
        outbox: Arc<InMemoryOutbox>,
        clock: Arc<dyn Clock>,
        message_for: SceneMessageMapping,
    ) -> Self {
        Self {
            enqueuing: Some((outbox, Mapping::new(clock, message_for))),
            ..self
        }
    }

    fn enqueue_change(&self, transaction: &mut Transaction, change: SceneChange) {
        if let Some((outbox, mapping)) = &self.enqueuing {
            outbox.enqueue_after_commit(transaction, mapping.message(&change));
        }
    }

    fn undo_on_rollback(&self, transaction: &mut Transaction) -> Result<(), StoreError> {
        let (before, scenes) = (self.read().clone(), self.scenes.clone());

        transacting::memory::undo_on_rollback(transaction, move || {
            *scenes.write().expect("scene store lock poisoned") = before;
        })
        .map_err(|failure| StoreError::Backend(Box::new(failure)))
    }

    fn read(&self) -> RwLockReadGuard<'_, Scenes> {
        self.scenes.read().expect("scene store lock poisoned")
    }

    fn write(&self) -> RwLockWriteGuard<'_, Scenes> {
        self.scenes.write().expect("scene store lock poisoned")
    }
}

fn rehydrate(id: SceneId, stored: &StoredScene) -> Result<Scene, StoreError> {
    Scene::rehydrate(id, stored.project.clone(), &stored.state)
        .map(|scene| {
            scene
                .titled(stored.title.clone())
                .linked_to(stored.ideas.clone())
        })
        .map_err(|reason| StoreError::Backend(Box::new(reason)))
}

#[async_trait]
impl SceneStore for InMemorySceneStore {
    async fn create(&self, transaction: &mut Transaction, scene: &Scene) -> Result<(), StoreError> {
        self.undo_on_rollback(transaction)?;
        let mut scenes = self.write();

        if scenes.contains_key(&scene.id()) {
            return Err(StoreError::Conflict(scene.id()));
        }

        scenes.insert(
            scene.id(),
            StoredScene {
                project: scene.project().clone(),
                title: scene.title().clone(),
                ideas: scene.ideas().to_vec(),
                version: 0,
                prose_version: 0,
                state: scene.everything(),
                uncounted: false,
            },
        );
        self.enqueue_change(
            transaction,
            SceneChange::Created {
                scene: scene.id(),
                project: scene.project().clone(),
            },
        );

        Ok(())
    }

    async fn load(&self, id: SceneId) -> Result<Scene, StoreError> {
        let stored = self
            .read()
            .get(&id)
            .cloned()
            .ok_or(StoreError::NotFound(id))?;

        rehydrate(id, &stored)
    }

    async fn apply(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        update: &[u8],
    ) -> Result<(), StoreError> {
        self.undo_on_rollback(transaction)?;
        let mut scenes = self.write();

        let stored = scenes.get(&id).cloned().ok_or(StoreError::NotFound(id))?;
        let scene = rehydrate(id, &stored)?;
        scene.apply(update).map_err(|_| StoreError::Unusable(id))?;

        scenes.insert(
            id,
            StoredScene {
                project: stored.project,
                title: stored.title,
                ideas: stored.ideas,
                version: stored.version,
                prose_version: stored.prose_version + 1,
                state: scene.everything(),
                uncounted: true,
            },
        );

        Ok(())
    }

    async fn retitle(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        title: &SceneTitle,
    ) -> Result<(), StoreError> {
        self.undo_on_rollback(transaction)?;
        let mut scenes = self.write();
        let stored = scenes.get_mut(&id).ok_or(StoreError::NotFound(id))?;
        stored.title = title.clone();
        stored.version += 1;
        self.enqueue_change(
            transaction,
            SceneChange::Retitled {
                scene: id,
                project: stored.project.clone(),
                title: title.clone(),
                version: stored.version,
            },
        );

        Ok(())
    }

    async fn announce_opening(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        opening: &str,
    ) -> Result<(), StoreError> {
        self.undo_on_rollback(transaction)?;
        let project = self
            .read()
            .get(&id)
            .map(|stored| stored.project.clone())
            .ok_or(StoreError::NotFound(id))?;
        self.enqueue_change(
            transaction,
            SceneChange::OpeningChanged {
                scene: id,
                project,
                opening: opening.to_owned(),
            },
        );

        Ok(())
    }

    async fn announce_word_count(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        words: usize,
        as_of: u64,
    ) -> Result<(), StoreError> {
        self.undo_on_rollback(transaction)?;
        let project = self
            .write()
            .get_mut(&id)
            .map(|stored| {
                if stored.prose_version == as_of {
                    stored.uncounted = false;
                }
                stored.project.clone()
            })
            .ok_or(StoreError::NotFound(id))?;
        self.enqueue_change(
            transaction,
            SceneChange::WordsCounted {
                scene: id,
                project,
                words,
            },
        );

        Ok(())
    }

    async fn link(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        idea: &IdeaLink,
    ) -> Result<(), StoreError> {
        self.undo_on_rollback(transaction)?;
        let mut scenes = self.write();
        let stored = scenes.get_mut(&id).ok_or(StoreError::NotFound(id))?;

        if !stored.ideas.contains(idea) {
            stored.ideas.push(idea.clone());
            stored.version += 1;
            self.enqueue_change(
                transaction,
                SceneChange::IdeaLinked {
                    scene: id,
                    project: stored.project.clone(),
                    idea: idea.clone(),
                    version: stored.version,
                },
            );
        }

        Ok(())
    }

    async fn unlink(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        idea: &IdeaLink,
    ) -> Result<(), StoreError> {
        self.undo_on_rollback(transaction)?;
        let mut scenes = self.write();
        let stored = scenes.get_mut(&id).ok_or(StoreError::NotFound(id))?;
        let before = stored.ideas.len();
        stored.ideas.retain(|held| held != idea);

        if stored.ideas.len() < before {
            stored.version += 1;
            self.enqueue_change(
                transaction,
                SceneChange::IdeaUnlinked {
                    scene: id,
                    project: stored.project.clone(),
                    idea: idea.clone(),
                    version: stored.version,
                },
            );
        }

        Ok(())
    }

    async fn unlink_everywhere(
        &self,
        transaction: &mut Transaction,
        idea: &IdeaLink,
    ) -> Result<(), StoreError> {
        self.undo_on_rollback(transaction)?;
        for stored in self.write().values_mut() {
            stored.ideas.retain(|held| held != idea);
        }

        Ok(())
    }

    async fn in_project(
        &self,
        project: &ProjectLink,
        after: Option<SceneId>,
        at_most: usize,
    ) -> Result<Vec<SceneId>, StoreError> {
        let mut found: Vec<SceneId> = self
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

    async fn version_of(&self, id: SceneId) -> Result<u64, StoreError> {
        self.read()
            .get(&id)
            .map(|stored| stored.version)
            .ok_or(StoreError::NotFound(id))
    }

    async fn prose_version_of(&self, id: SceneId) -> Result<u64, StoreError> {
        self.read()
            .get(&id)
            .map(|stored| stored.prose_version)
            .ok_or(StoreError::NotFound(id))
    }

    async fn version_within(
        &self,
        _transaction: &mut Transaction,
        id: SceneId,
    ) -> Result<u64, StoreError> {
        self.version_of(id).await
    }

    async fn uncounted(&self, at_most: usize) -> Result<Vec<SceneId>, StoreError> {
        let mut found: Vec<SceneId> = self
            .read()
            .iter()
            .filter(|(_, stored)| stored.uncounted)
            .map(|(id, _)| *id)
            .collect();
        found.sort();
        found.truncate(at_most);

        Ok(found)
    }

    async fn delete(&self, transaction: &mut Transaction, id: SceneId) -> Result<(), StoreError> {
        self.undo_on_rollback(transaction)?;
        let mut scenes = self.write();
        let gone = scenes.remove(&id).ok_or(StoreError::NotFound(id))?;
        self.enqueue_change(
            transaction,
            SceneChange::Deleted {
                scene: id,
                project: gone.project,
            },
        );

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use clock::FixedClock;
    use messaging::{FakePublisher, Message};
    use outbox::Outbox;
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;
    use crate::suite;
    use crate::suite::Workbench;

    struct InMemory {
        store: InMemorySceneStore,
        transactions: InMemoryTransactions,
        outbox: Arc<InMemoryOutbox>,
        heard: Arc<FakePublisher>,
    }

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemorySceneStore;

        async fn setup() -> Self {
            let clock = Arc::new(FixedClock::new(suite::at(2_000)));
            let heard = Arc::new(FakePublisher::default());
            let outbox = Arc::new(InMemoryOutbox::new(heard.clone(), clock.clone()));

            Self {
                transactions: InMemoryTransactions::new(),
                store: InMemorySceneStore::new().enqueuing_to(
                    outbox.clone(),
                    clock,
                    suite::message_for,
                ),
                outbox,
                heard,
            }
        }

        fn store(&self) -> &Self::Store {
            &self.store
        }

        fn transactions(&self) -> &dyn Transactions {
            &self.transactions
        }

        async fn enqueued(&self) -> Vec<Message> {
            self.outbox
                .deliver(100)
                .await
                .expect("the in-memory outbox always delivers");

            self.heard.heard()
        }

        async fn cleanup(self) {}
    }

    crate::suite::conformance_tests!(InMemory);
}
