use std::sync::Arc;

use authorizing::{Act, Authorizer};
use clock::Clock;
use eventsourcing::Agent;

use crate::{
    ProjectLink, QuotationRestylingProgress, QuotationRestylingTasks, Scene, SceneCatalog, SceneId,
    SceneService, SceneStore, SceneSummary, ScenesError, StoreError,
};

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Access {
    Author,
    Reader,
    Stranger,
}

#[derive(Clone)]
pub struct ScenesCore {
    service: SceneService,
    catalog: Arc<dyn SceneCatalog>,
    tasks: Arc<dyn QuotationRestylingTasks>,
    authorizer: Arc<dyn Authorizer>,
}

impl ScenesCore {
    pub fn new(
        store: Arc<dyn SceneStore>,
        catalog: Arc<dyn SceneCatalog>,
        tasks: Arc<dyn QuotationRestylingTasks>,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            service: SceneService::new(store, clock),
            catalog,
            tasks,
            authorizer,
        }
    }

    pub async fn create(&self, project: &str, agent: &Agent) -> Result<Scene, ScenesError> {
        self.authorized(agent, Act::Write, &ProjectLink::from(project), || {
            ScenesError::ProjectNotThere
        })
        .await?;

        self.service.create(project).await
    }

    pub async fn read(&self, id: &str, agent: &Agent) -> Result<Scene, ScenesError> {
        let scene = self.service.open(id).await?;
        self.authorized(agent, Act::Read, scene.project(), || not_there(&scene))
            .await?;

        Ok(scene)
    }

    pub async fn retitle(
        &self,
        id: &str,
        title: &str,
        agent: &Agent,
    ) -> Result<Scene, ScenesError> {
        self.authorized_to_write(id, agent).await?;

        self.service.retitle(id, title).await
    }

    pub async fn link(&self, id: &str, idea: &str, agent: &Agent) -> Result<Scene, ScenesError> {
        self.authorized_to_write(id, agent).await?;

        self.service.link(id, idea).await
    }

    pub async fn unlink(&self, id: &str, idea: &str, agent: &Agent) -> Result<Scene, ScenesError> {
        self.authorized_to_write(id, agent).await?;

        self.service.unlink(id, idea).await
    }

    pub async fn delete(&self, id: &str, agent: &Agent) -> Result<(), ScenesError> {
        self.authorized_to_write(id, agent).await?;

        self.service.delete(id).await
    }

    pub async fn list(
        &self,
        project: &str,
        after: Option<&str>,
        at_most: usize,
        agent: &Agent,
    ) -> Result<Vec<SceneSummary>, ScenesError> {
        let project = self.authorized_to_read(project, agent).await?;
        let after = after.map(str::parse).transpose()?;

        Ok(self
            .catalog
            .in_project_after(&project, after, at_most)
            .await?)
    }

    pub async fn restyling_progress(
        &self,
        project: &str,
        agent: &Agent,
    ) -> Result<Option<QuotationRestylingProgress>, ScenesError> {
        let project = self.authorized_to_read(project, agent).await?;

        Ok(self.tasks.progress(&project).await?)
    }

    pub async fn may_follow(&self, project: &str, agent: &Agent) -> Result<bool, ScenesError> {
        Ok(self.authorizer.may(agent, Act::Read, project).await?)
    }

    pub async fn access(&self, scene: &Scene, agent: &Agent) -> Result<Access, ScenesError> {
        let project = scene.project().as_str();

        if self.authorizer.may(agent, Act::Write, project).await? {
            Ok(Access::Author)
        } else if self.authorizer.may(agent, Act::Read, project).await? {
            Ok(Access::Reader)
        } else {
            Ok(Access::Stranger)
        }
    }

    pub async fn open(&self, id: &str) -> Result<Scene, ScenesError> {
        self.service.open(id).await
    }

    pub async fn apply(&self, id: &str, update: &[u8]) -> Result<(), ScenesError> {
        self.service.apply(id, update).await
    }

    pub async fn announce_word_count(&self, id: &str, words: usize) -> Result<(), ScenesError> {
        self.service.announce_word_count(id, words).await
    }

    pub async fn announce_opening(&self, id: &str, opening: &str) -> Result<(), ScenesError> {
        self.service.announce_opening(id, opening).await
    }

    pub async fn sweep(
        &self,
        project: &str,
        after: Option<&str>,
        at_most: usize,
    ) -> Result<Vec<SceneId>, ScenesError> {
        let batch = self.service.in_project(project, after, at_most).await?;

        for scene in &batch {
            match self.delete(&scene.to_string(), &Agent::System).await {
                Ok(()) | Err(ScenesError::Store(StoreError::NotFound(_))) => {}
                Err(refused) => return Err(refused),
            }
        }

        Ok(batch)
    }

    async fn authorized_to_read(
        &self,
        project: &str,
        agent: &Agent,
    ) -> Result<ProjectLink, ScenesError> {
        if self.may_follow(project, agent).await? {
            Ok(ProjectLink::from(project))
        } else {
            Err(ScenesError::ProjectNotThere)
        }
    }

    async fn authorized_to_write(&self, id: &str, agent: &Agent) -> Result<(), ScenesError> {
        let scene = self.service.open(id).await?;

        self.authorized(agent, Act::Write, scene.project(), || not_there(&scene))
            .await
    }

    async fn authorized(
        &self,
        agent: &Agent,
        act: Act,
        project: &ProjectLink,
        otherwise: impl FnOnce() -> ScenesError,
    ) -> Result<(), ScenesError> {
        if self.authorizer.may(agent, act, project.as_str()).await? {
            Ok(())
        } else if self
            .authorizer
            .may(agent, Act::Read, project.as_str())
            .await?
        {
            Err(ScenesError::NotAnAuthor)
        } else {
            Err(otherwise())
        }
    }
}

fn not_there(scene: &Scene) -> ScenesError {
    StoreError::NotFound(scene.id()).into()
}
