use std::sync::Arc;

use clock::Clock;
use ids::InvalidId;
use thiserror::Error;

use crate::{
    IdeaLink, InvalidSceneTitle, ProjectLink, Scene, SceneId, SceneStore, SceneTitle, StoreError,
};

#[derive(Debug, Error)]
pub enum SceneServiceError {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    Untitled(#[from] InvalidSceneTitle),
    #[error(transparent)]
    Store(#[from] StoreError),
}

#[derive(Clone)]
pub struct SceneService {
    store: Arc<dyn SceneStore>,
    clock: Arc<dyn Clock>,
}

impl SceneService {
    pub fn new(store: Arc<dyn SceneStore>, clock: Arc<dyn Clock>) -> Self {
        Self { store, clock }
    }

    pub async fn create(&self, project: &str) -> Result<Scene, SceneServiceError> {
        let scene = Scene::empty(
            SceneId::generate(self.clock.now()),
            ProjectLink::from(project),
        );

        self.store.create(&scene).await?;

        Ok(scene)
    }

    pub async fn open(&self, id: &str) -> Result<Scene, SceneServiceError> {
        let id: SceneId = id.parse()?;

        Ok(self.store.load(id).await?)
    }

    pub async fn apply(&self, id: &str, update: &[u8]) -> Result<(), SceneServiceError> {
        let id: SceneId = id.parse()?;

        Ok(self.store.apply(id, update).await?)
    }

    pub async fn in_project(
        &self,
        project: &str,
        after: Option<&str>,
        at_most: usize,
    ) -> Result<Vec<SceneId>, SceneServiceError> {
        let after = after.map(str::parse).transpose()?;

        Ok(self
            .store
            .in_project(&ProjectLink::from(project), after, at_most)
            .await?)
    }

    pub async fn retitle(&self, id: &str, title: &str) -> Result<Scene, SceneServiceError> {
        let id: SceneId = id.parse()?;
        let title = SceneTitle::new(title)?;

        self.store.retitle(id, &title).await?;

        Ok(self.store.load(id).await?)
    }

    pub async fn link(&self, id: &str, idea: &str) -> Result<Scene, SceneServiceError> {
        let id: SceneId = id.parse()?;

        self.store.link(id, &IdeaLink::from(idea)).await?;

        Ok(self.store.load(id).await?)
    }

    pub async fn unlink(&self, id: &str, idea: &str) -> Result<Scene, SceneServiceError> {
        let id: SceneId = id.parse()?;

        self.store.unlink(id, &IdeaLink::from(idea)).await?;

        Ok(self.store.load(id).await?)
    }

    pub async fn unlink_everywhere(&self, idea: &str) -> Result<(), SceneServiceError> {
        Ok(self.store.unlink_everywhere(&IdeaLink::from(idea)).await?)
    }

    pub async fn delete(&self, id: &str) -> Result<(), SceneServiceError> {
        let id: SceneId = id.parse()?;

        Ok(self.store.delete(id).await?)
    }
}
