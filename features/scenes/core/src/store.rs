use async_trait::async_trait;
use thiserror::Error;
use transacting::Transaction;

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
    async fn create(&self, transaction: &mut Transaction, scene: &Scene) -> Result<(), StoreError>;

    async fn load(&self, id: SceneId) -> Result<Scene, StoreError>;

    async fn apply(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        update: &[u8],
    ) -> Result<(), StoreError>;

    async fn retitle(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        title: &SceneTitle,
    ) -> Result<(), StoreError>;

    async fn announce_word_count(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        words: usize,
    ) -> Result<(), StoreError>;

    async fn announce_opening(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        opening: &str,
    ) -> Result<(), StoreError>;

    async fn link(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        idea: &IdeaLink,
    ) -> Result<(), StoreError>;

    async fn unlink(
        &self,
        transaction: &mut Transaction,
        id: SceneId,
        idea: &IdeaLink,
    ) -> Result<(), StoreError>;

    async fn unlink_everywhere(
        &self,
        transaction: &mut Transaction,
        idea: &IdeaLink,
    ) -> Result<(), StoreError>;

    async fn delete(&self, transaction: &mut Transaction, id: SceneId) -> Result<(), StoreError>;

    async fn in_project(
        &self,
        project: &ProjectLink,
        after: Option<SceneId>,
        at_most: usize,
    ) -> Result<Vec<SceneId>, StoreError>;

    async fn uncounted(&self, at_most: usize) -> Result<Vec<SceneId>, StoreError>;
}
