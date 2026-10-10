use async_trait::async_trait;
use thiserror::Error;

use crate::{IdeaLink, ProjectLink, Scene, SceneId, SceneTitle};

#[derive(Debug, Error)]
pub enum StoreError {
    #[error("scene {0} was not found")]
    NotFound(SceneId),
    #[error("scene {0} already exists")]
    Conflict(SceneId),
    #[error("the update offered to scene {0} could not be applied")]
    Unusable(SceneId),
    #[error("the scene store failed: {0}")]
    Backend(#[source] Box<dyn std::error::Error + Send + Sync>),
}

#[async_trait]
pub trait SceneStore: Send + Sync {
    async fn create(&self, scene: &Scene) -> Result<(), StoreError>;

    async fn load(&self, id: SceneId) -> Result<Scene, StoreError>;

    async fn apply(&self, id: SceneId, update: &[u8]) -> Result<(), StoreError>;

    async fn retitle(&self, id: SceneId, title: &SceneTitle) -> Result<(), StoreError>;

    async fn link(&self, id: SceneId, idea: &IdeaLink) -> Result<(), StoreError>;

    async fn unlink(&self, id: SceneId, idea: &IdeaLink) -> Result<(), StoreError>;

    async fn unlink_everywhere(&self, idea: &IdeaLink) -> Result<(), StoreError>;

    async fn delete(&self, id: SceneId) -> Result<(), StoreError>;

    async fn in_project(
        &self,
        project: &ProjectLink,
        after: Option<SceneId>,
        at_most: usize,
    ) -> Result<Vec<SceneId>, StoreError>;
}
