use std::sync::Arc;

use authorizing::{Act, Authorizer, AuthorizerError};
use clock::Clock;
use eventsourcing::Agent;
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
    #[error(transparent)]
    Authorizer(#[from] AuthorizerError),
    #[error("there is no such project")]
    ProjectNotThere,
    #[error("only an author of the project may do this")]
    NotAnAuthor,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Author,
    Reader,
    Stranger,
}

#[derive(Clone)]
pub struct SceneService {
    store: Arc<dyn SceneStore>,
    authorizer: Arc<dyn Authorizer>,
    clock: Arc<dyn Clock>,
}

impl SceneService {
    pub fn new(
        store: Arc<dyn SceneStore>,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            store,
            authorizer,
            clock,
        }
    }

    pub async fn create(&self, project: &str, agent: &Agent) -> Result<Scene, SceneServiceError> {
        self.admitted(agent, Act::Write, &ProjectLink::from(project), || {
            SceneServiceError::ProjectNotThere
        })
        .await?;
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

    pub async fn read(&self, id: &str, agent: &Agent) -> Result<Scene, SceneServiceError> {
        let id: SceneId = id.parse()?;
        let scene = self.store.load(id).await?;
        self.admitted(agent, Act::Read, scene.project(), || {
            StoreError::NotFound(id).into()
        })
        .await?;

        Ok(scene)
    }

    pub async fn access(
        &self,
        project: &ProjectLink,
        agent: &Agent,
    ) -> Result<Access, SceneServiceError> {
        if self
            .authorizer
            .may(agent, Act::Write, project.as_str())
            .await?
        {
            Ok(Access::Author)
        } else if self
            .authorizer
            .may(agent, Act::Read, project.as_str())
            .await?
        {
            Ok(Access::Reader)
        } else {
            Ok(Access::Stranger)
        }
    }

    async fn written_by(&self, id: SceneId, agent: &Agent) -> Result<(), SceneServiceError> {
        let scene = self.store.load(id).await?;

        self.admitted(agent, Act::Write, scene.project(), || {
            StoreError::NotFound(id).into()
        })
        .await
    }

    async fn admitted(
        &self,
        agent: &Agent,
        act: Act,
        project: &ProjectLink,
        otherwise: impl FnOnce() -> SceneServiceError,
    ) -> Result<(), SceneServiceError> {
        if self.authorizer.may(agent, act, project.as_str()).await? {
            Ok(())
        } else if self
            .authorizer
            .may(agent, Act::Read, project.as_str())
            .await?
        {
            Err(SceneServiceError::NotAnAuthor)
        } else {
            Err(otherwise())
        }
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

    pub async fn retitle(
        &self,
        id: &str,
        title: &str,
        agent: &Agent,
    ) -> Result<Scene, SceneServiceError> {
        let id: SceneId = id.parse()?;
        self.written_by(id, agent).await?;
        let title = SceneTitle::new(title)?;

        self.store.retitle(id, &title).await?;

        Ok(self.store.load(id).await?)
    }

    pub async fn announce_word_count(
        &self,
        id: &str,
        words: usize,
    ) -> Result<(), SceneServiceError> {
        let id: SceneId = id.parse()?;

        Ok(self.store.announce_word_count(id, words).await?)
    }

    pub async fn announce_opening(&self, id: &str, opening: &str) -> Result<(), SceneServiceError> {
        let id: SceneId = id.parse()?;

        Ok(self.store.announce_opening(id, opening).await?)
    }

    pub async fn link(
        &self,
        id: &str,
        idea: &str,
        agent: &Agent,
    ) -> Result<Scene, SceneServiceError> {
        let id: SceneId = id.parse()?;
        self.written_by(id, agent).await?;

        self.store.link(id, &IdeaLink::from(idea)).await?;

        Ok(self.store.load(id).await?)
    }

    pub async fn unlink(
        &self,
        id: &str,
        idea: &str,
        agent: &Agent,
    ) -> Result<Scene, SceneServiceError> {
        let id: SceneId = id.parse()?;
        self.written_by(id, agent).await?;

        self.store.unlink(id, &IdeaLink::from(idea)).await?;

        Ok(self.store.load(id).await?)
    }

    pub async fn unlink_everywhere(&self, idea: &str) -> Result<(), SceneServiceError> {
        Ok(self.store.unlink_everywhere(&IdeaLink::from(idea)).await?)
    }

    pub async fn delete(&self, id: &str, agent: &Agent) -> Result<(), SceneServiceError> {
        let id: SceneId = id.parse()?;
        self.written_by(id, agent).await?;

        Ok(self.store.delete(id).await?)
    }
}
