use std::sync::Arc;

use clock::Clock;

use crate::{IdeaLink, ProjectLink, Scene, SceneId, SceneStore, SceneTitle, ScenesError};

#[derive(Clone)]
pub struct SceneService {
    store: Arc<dyn SceneStore>,
    clock: Arc<dyn Clock>,
}

impl SceneService {
    pub fn new(store: Arc<dyn SceneStore>, clock: Arc<dyn Clock>) -> Self {
        Self { store, clock }
    }

    pub async fn create(&self, project: &str) -> Result<Scene, ScenesError> {
        let scene = Scene::empty(
            SceneId::generate(self.clock.now()),
            ProjectLink::from(project),
        );

        self.store.create(&scene).await?;

        Ok(scene)
    }

    pub async fn open(&self, id: &str) -> Result<Scene, ScenesError> {
        let id: SceneId = id.parse()?;

        Ok(self.store.load(id).await?)
    }

    pub async fn apply(&self, id: &str, update: &[u8]) -> Result<(), ScenesError> {
        let id: SceneId = id.parse()?;

        Ok(self.store.apply(id, update).await?)
    }

    pub async fn in_project(
        &self,
        project: &str,
        after: Option<&str>,
        at_most: usize,
    ) -> Result<Vec<SceneId>, ScenesError> {
        let after = after.map(str::parse).transpose()?;

        Ok(self
            .store
            .in_project(&ProjectLink::from(project), after, at_most)
            .await?)
    }

    pub async fn retitle(&self, id: &str, title: &str) -> Result<Scene, ScenesError> {
        let id: SceneId = id.parse()?;
        let title = SceneTitle::new(title)?;

        self.store.retitle(id, &title).await?;

        Ok(self.store.load(id).await?)
    }

    pub async fn announce_word_count(&self, id: &str, words: usize) -> Result<(), ScenesError> {
        let id: SceneId = id.parse()?;

        Ok(self.store.announce_word_count(id, words).await?)
    }

    pub async fn announce_opening(&self, id: &str, opening: &str) -> Result<(), ScenesError> {
        let id: SceneId = id.parse()?;

        Ok(self.store.announce_opening(id, opening).await?)
    }

    pub async fn link(&self, id: &str, idea: &str) -> Result<Scene, ScenesError> {
        let id: SceneId = id.parse()?;

        self.store.link(id, &IdeaLink::from(idea)).await?;

        Ok(self.store.load(id).await?)
    }

    pub async fn unlink(&self, id: &str, idea: &str) -> Result<Scene, ScenesError> {
        let id: SceneId = id.parse()?;

        self.store.unlink(id, &IdeaLink::from(idea)).await?;

        Ok(self.store.load(id).await?)
    }

    pub async fn unlink_everywhere(&self, idea: &str) -> Result<(), ScenesError> {
        Ok(self.store.unlink_everywhere(&IdeaLink::from(idea)).await?)
    }

    pub async fn delete(&self, id: &str) -> Result<(), ScenesError> {
        let id: SceneId = id.parse()?;

        Ok(self.store.delete(id).await?)
    }
}
