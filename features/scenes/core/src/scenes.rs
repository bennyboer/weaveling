use std::sync::Arc;

use authorizing::{Act, Authorizer};
use clock::Clock;
use eventsourcing::Agent;
use time::OffsetDateTime;

use crate::service::SceneService;
use crate::{
    ProjectLink, QuotationRestyling, QuotationRestylingProgress, QuotationRestylingTasks, Scene,
    SceneCatalog, SceneId, SceneRestyler, SceneStore, SceneSummary, SceneTitle, ScenesError,
    StoreError,
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

    pub async fn unlink_everywhere(&self, idea: &str) -> Result<(), ScenesError> {
        self.service.unlink_everywhere(idea).await
    }

    pub async fn restyle(
        &self,
        restyling: &QuotationRestyling,
        at_most: usize,
        restyler: &dyn SceneRestyler,
    ) -> Result<Option<SceneId>, ScenesError> {
        let project = ProjectLink::from(restyling.project.as_str());
        if !self.claimed(&project, restyling, at_most).await? {
            return Ok(None);
        }
        let batch = self
            .service
            .in_project(&restyling.project, restyling.after.as_deref(), at_most)
            .await?;

        for scene in &batch {
            match restyler.restyle(*scene, restyling.style).await {
                Ok(_) | Err(ScenesError::Store(StoreError::NotFound(_))) => {}
                Err(refused) => return Err(refused),
            }
        }

        let finished = batch.len() < at_most;
        self.tasks
            .progressed(&project, restyling.change, in_count(batch.len()), finished)
            .await?;

        Ok(if finished {
            None
        } else {
            batch.last().copied()
        })
    }

    pub async fn catalogue_created(&self, scene: &str, project: &str) -> Result<(), ScenesError> {
        Ok(self
            .catalog
            .remember(scene.parse()?, &ProjectLink::from(project))
            .await?)
    }

    pub async fn catalogue_retitled(
        &self,
        scene: &str,
        project: &str,
        title: &str,
        version: u64,
    ) -> Result<(), ScenesError> {
        Ok(self
            .catalog
            .retitle(
                scene.parse()?,
                &ProjectLink::from(project),
                &SceneTitle::new(title)?,
                version,
            )
            .await?)
    }

    pub async fn catalogue_opening(
        &self,
        scene: &str,
        project: &str,
        opening: &str,
        at: OffsetDateTime,
    ) -> Result<(), ScenesError> {
        Ok(self
            .catalog
            .note_opening(scene.parse()?, &ProjectLink::from(project), opening, at)
            .await?)
    }

    pub async fn catalogue_deleted(
        &self,
        scene: &str,
        at: OffsetDateTime,
    ) -> Result<(), ScenesError> {
        Ok(self.catalog.forget(scene.parse()?, at).await?)
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, ScenesError> {
        Ok(self
            .catalog
            .forget_tombstones_laid_before(before, at_most)
            .await?)
    }

    async fn claimed(
        &self,
        project: &ProjectLink,
        restyling: &QuotationRestyling,
        at_most: usize,
    ) -> Result<bool, ScenesError> {
        if restyling.after.is_some() {
            return Ok(self.tasks.claim(project, restyling.change).await?);
        }
        let total = self.scenes_in(&restyling.project, at_most).await?;

        Ok(self.tasks.begin(project, restyling.change, total).await?)
    }

    async fn scenes_in(&self, project: &str, at_most: usize) -> Result<u64, ScenesError> {
        let mut total = 0;
        let mut after: Option<String> = None;

        loop {
            let page = self
                .service
                .in_project(project, after.as_deref(), at_most)
                .await?;
            total += in_count(page.len());
            match page.last() {
                Some(last) if page.len() == at_most => after = Some(last.to_string()),
                _ => return Ok(total),
            }
        }
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

fn in_count(scenes: usize) -> u64 {
    u64::try_from(scenes).expect("a number of scenes fits 64 bits")
}

fn not_there(scene: &Scene) -> ScenesError {
    StoreError::NotFound(scene.id()).into()
}
