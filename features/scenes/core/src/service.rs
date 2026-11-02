use std::sync::Arc;

use clock::Clock;
use transacting::Transaction;

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

    pub async fn create(
        &self,
        transaction: &mut Transaction,
        project: &str,
    ) -> Result<Scene, ScenesError> {
        let scene = Scene::empty(
            SceneId::generate(self.clock.now()),
            ProjectLink::from(project),
        );

        self.store.create(transaction, &scene).await?;

        Ok(scene)
    }

    pub async fn open(&self, id: &str) -> Result<Scene, ScenesError> {
        let id: SceneId = id.parse()?;

        Ok(self.store.load(id).await?)
    }

    pub async fn apply(
        &self,
        transaction: &mut Transaction,
        id: &str,
        update: &[u8],
    ) -> Result<(), ScenesError> {
        let id: SceneId = id.parse()?;

        Ok(self.store.apply(transaction, id, update).await?)
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

    pub async fn retitle(
        &self,
        transaction: &mut Transaction,
        id: &str,
        title: &str,
    ) -> Result<(), ScenesError> {
        let id: SceneId = id.parse()?;
        let title = SceneTitle::new(title)?;

        Ok(self.store.retitle(transaction, id, &title).await?)
    }

    pub async fn announce_word_count(
        &self,
        transaction: &mut Transaction,
        id: &str,
        words: usize,
    ) -> Result<(), ScenesError> {
        let id: SceneId = id.parse()?;

        Ok(self
            .store
            .announce_word_count(transaction, id, words)
            .await?)
    }

    pub async fn announce_opening(
        &self,
        transaction: &mut Transaction,
        id: &str,
        opening: &str,
    ) -> Result<(), ScenesError> {
        let id: SceneId = id.parse()?;

        Ok(self
            .store
            .announce_opening(transaction, id, opening)
            .await?)
    }

    pub async fn link(
        &self,
        transaction: &mut Transaction,
        id: &str,
        idea: &str,
    ) -> Result<(), ScenesError> {
        let id: SceneId = id.parse()?;

        Ok(self
            .store
            .link(transaction, id, &IdeaLink::from(idea))
            .await?)
    }

    pub async fn unlink(
        &self,
        transaction: &mut Transaction,
        id: &str,
        idea: &str,
    ) -> Result<(), ScenesError> {
        let id: SceneId = id.parse()?;

        Ok(self
            .store
            .unlink(transaction, id, &IdeaLink::from(idea))
            .await?)
    }

    pub async fn unlink_everywhere(
        &self,
        transaction: &mut Transaction,
        idea: &str,
    ) -> Result<(), ScenesError> {
        Ok(self
            .store
            .unlink_everywhere(transaction, &IdeaLink::from(idea))
            .await?)
    }

    pub async fn delete(&self, transaction: &mut Transaction, id: &str) -> Result<(), ScenesError> {
        let id: SceneId = id.parse()?;

        Ok(self.store.delete(transaction, id).await?)
    }
}
