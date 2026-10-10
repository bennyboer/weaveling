use std::collections::HashMap;
use std::sync::{Arc, RwLock, RwLockReadGuard, RwLockWriteGuard};

use crate::enqueuing::{Mapping, SceneMessageMapping};
use async_trait::async_trait;
use clock::Clock;
use outbox::InMemoryOutbox;
use scenes_core::{
    IdeaLink, ProjectLink, Scene, SceneChange, SceneId, SceneStore, SceneTitle, StoreError,
};

type Scenes = HashMap<SceneId, StoredScene>;

#[derive(Debug, Clone)]
struct StoredScene {
    project: ProjectLink,
    title: SceneTitle,
    ideas: Vec<IdeaLink>,
    version: u64,
    state: Vec<u8>,
}

#[derive(Default)]
pub struct InMemorySceneStore {
    scenes: RwLock<Scenes>,
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

    fn enqueue_change(&self, change: SceneChange) {
        if let Some((outbox, mapping)) = &self.enqueuing {
            outbox.enqueue(mapping.message(&change));
        }
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
    async fn create(&self, scene: &Scene) -> Result<(), StoreError> {
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
                state: scene.everything(),
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

    async fn apply(&self, id: SceneId, update: &[u8]) -> Result<(), StoreError> {
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
                version: stored.version + 1,
                state: scene.everything(),
            },
        );

        Ok(())
    }

    async fn retitle(&self, id: SceneId, title: &SceneTitle) -> Result<(), StoreError> {
        let mut scenes = self.write();
        let stored = scenes.get_mut(&id).ok_or(StoreError::NotFound(id))?;
        stored.title = title.clone();
        stored.version += 1;

        Ok(())
    }

    async fn link(&self, id: SceneId, idea: &IdeaLink) -> Result<(), StoreError> {
        let mut scenes = self.write();
        let stored = scenes.get_mut(&id).ok_or(StoreError::NotFound(id))?;

        if !stored.ideas.contains(idea) {
            stored.ideas.push(idea.clone());
            stored.version += 1;
            self.enqueue_change(SceneChange::IdeaLinked {
                scene: id,
                idea: idea.clone(),
                version: stored.version,
            });
        }

        Ok(())
    }

    async fn unlink(&self, id: SceneId, idea: &IdeaLink) -> Result<(), StoreError> {
        let mut scenes = self.write();
        let stored = scenes.get_mut(&id).ok_or(StoreError::NotFound(id))?;
        let before = stored.ideas.len();
        stored.ideas.retain(|held| held != idea);

        if stored.ideas.len() < before {
            stored.version += 1;
            self.enqueue_change(SceneChange::IdeaUnlinked {
                scene: id,
                idea: idea.clone(),
                version: stored.version,
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

    async fn delete(&self, id: SceneId) -> Result<(), StoreError> {
        let mut scenes = self.write();
        scenes.remove(&id).ok_or(StoreError::NotFound(id))?;
        self.enqueue_change(SceneChange::Deleted { scene: id });

        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use clock::FixedClock;
    use messaging::Message;
    use outbox::Outbox;

    use super::*;
    use crate::suite::{Heard, Workbench, at, message_for};

    struct InMemory {
        store: InMemorySceneStore,
        outbox: Arc<InMemoryOutbox>,
        heard: Arc<Heard>,
    }

    #[async_trait]
    impl Workbench for InMemory {
        type Store = InMemorySceneStore;

        async fn setup() -> Self {
            let clock = Arc::new(FixedClock::new(at(2_000)));
            let heard = Arc::new(Heard::default());
            let outbox = Arc::new(InMemoryOutbox::new(heard.clone(), clock.clone()));

            Self {
                store: InMemorySceneStore::new().enqueuing_to(outbox.clone(), clock, message_for),
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
