use std::sync::Arc;

use authorizing::{AccessWatch, Act, Authorizer};
use clock::Clock;
use eventsourcing::Agent;
use time::OffsetDateTime;
use transacting::{Transaction, Transactions};

use crate::service::SceneService;
use crate::{
    Page, ProjectLink, QuotationRestyling, QuotationRestylingProgress, QuotationRestylingTasks,
    Scene, SceneCatalog, SceneId, SceneRestyler, SceneStore, SceneSummary, SceneTitle, ScenesError,
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
    transactions: Arc<dyn Transactions>,
    catalog: Arc<dyn SceneCatalog>,
    tasks: Arc<dyn QuotationRestylingTasks>,
    authorizer: Arc<dyn Authorizer>,
}

impl ScenesCore {
    pub fn new(
        store: Arc<dyn SceneStore>,
        transactions: Arc<dyn Transactions>,
        catalog: Arc<dyn SceneCatalog>,
        tasks: Arc<dyn QuotationRestylingTasks>,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            service: SceneService::new(store, clock),
            transactions,
            catalog,
            tasks,
            authorizer,
        }
    }

    pub async fn create(&self, project: &str, agent: &Agent) -> Result<Scene, ScenesError> {
        let link = ProjectLink::from(project);
        self.authorized(agent, Act::Write, &link, || ScenesError::ProjectNotThere)
            .await?;
        if self.catalog.has_project_tombstone(&link).await? {
            return Err(ScenesError::ProjectNotThere);
        }

        let mut transaction = self.transactions.begin().await?;
        let scene = self.service.create(&mut transaction, project).await?;
        transaction.finish().await?;
        if self.catalog.has_project_tombstone(&link).await? {
            match self.delete(&scene.id().to_string(), &Agent::System).await {
                Ok(()) | Err(ScenesError::Store(StoreError::NotFound(_))) => {}
                Err(refused) => return Err(refused),
            }
            return Err(ScenesError::ProjectNotThere);
        }

        Ok(scene)
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

        let mut transaction = self.transactions.begin().await?;
        self.service.retitle(&mut transaction, id, title).await?;
        transaction.finish().await?;

        self.service.open(id).await
    }

    pub async fn link(&self, id: &str, idea: &str, agent: &Agent) -> Result<Scene, ScenesError> {
        self.authorized_to_write(id, agent).await?;

        let mut transaction = self.transactions.begin().await?;
        self.service.link(&mut transaction, id, idea).await?;
        transaction.finish().await?;

        self.service.open(id).await
    }

    pub async fn unlink(&self, id: &str, idea: &str, agent: &Agent) -> Result<Scene, ScenesError> {
        self.authorized_to_write(id, agent).await?;

        let mut transaction = self.transactions.begin().await?;
        self.service.unlink(&mut transaction, id, idea).await?;
        transaction.finish().await?;

        self.service.open(id).await
    }

    pub async fn delete(&self, id: &str, agent: &Agent) -> Result<(), ScenesError> {
        self.authorized_to_write(id, agent).await?;

        let mut transaction = self.transactions.begin().await?;
        self.service.delete(&mut transaction, id).await?;

        Ok(transaction.finish().await?)
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

    pub fn watch_access(&self, project: &str) -> AccessWatch {
        self.authorizer.watch_access(project)
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
        let mut transaction = self.transactions.begin().await?;
        self.service.apply(&mut transaction, id, update).await?;

        Ok(transaction.finish().await?)
    }

    pub async fn uncounted(&self, at_most: usize) -> Result<Vec<SceneId>, ScenesError> {
        self.service.uncounted(at_most).await
    }

    pub async fn announce_word_count(&self, id: &str, words: usize) -> Result<(), ScenesError> {
        let mut transaction = self.transactions.begin().await?;
        self.service
            .announce_word_count(&mut transaction, id, words)
            .await?;

        Ok(transaction.finish().await?)
    }

    pub async fn announce_opening(&self, id: &str, opening: &str) -> Result<(), ScenesError> {
        let mut transaction = self.transactions.begin().await?;
        self.service
            .announce_opening(&mut transaction, id, opening)
            .await?;

        Ok(transaction.finish().await?)
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

    pub async fn unlink_everywhere(
        &self,
        within: Option<&mut Transaction>,
        idea: &str,
    ) -> Result<(), ScenesError> {
        let mut transaction = self.transactions.join(within).await?;
        self.service
            .unlink_everywhere(&mut transaction, idea)
            .await?;

        Ok(transaction.finish().await?)
    }

    pub async fn restyle(
        &self,
        restyling: &QuotationRestyling,
        at_most: usize,
        restyler: &dyn SceneRestyler,
    ) -> Result<Option<SceneId>, ScenesError> {
        let project = ProjectLink::from(restyling.project.as_str());
        match self.claimed(&project, restyling, at_most).await? {
            Claimed::Page => {}
            Claimed::Passed(carried_on_from) => return Ok(carried_on_from),
            Claimed::Superseded => return Ok(None),
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
        let reached = batch
            .last()
            .map(ToString::to_string)
            .or_else(|| restyling.after.clone());
        let mut transaction = self.transactions.begin().await?;
        let counted = self
            .tasks
            .progressed(
                &mut transaction,
                &project,
                restyling.change,
                Page {
                    from: restyling.after.as_deref(),
                    to: reached.as_deref(),
                    done: in_count(batch.len()),
                },
                finished,
            )
            .await?;
        transaction.finish().await?;

        Ok(if counted && !finished {
            batch.last().copied()
        } else {
            None
        })
    }

    pub async fn catalogue_created(
        &self,
        within: Option<&mut Transaction>,
        scene: &str,
        project: &str,
    ) -> Result<(), ScenesError> {
        let mut transaction = self.transactions.join(within).await?;
        self.catalog
            .remember(
                &mut transaction,
                scene.parse()?,
                &ProjectLink::from(project),
            )
            .await?;

        Ok(transaction.finish().await?)
    }

    pub async fn catalogue_retitled(
        &self,
        within: Option<&mut Transaction>,
        scene: &str,
        project: &str,
        title: &str,
        version: u64,
    ) -> Result<(), ScenesError> {
        let mut transaction = self.transactions.join(within).await?;
        self.catalog
            .retitle(
                &mut transaction,
                scene.parse()?,
                &ProjectLink::from(project),
                &SceneTitle::new(title)?,
                version,
            )
            .await?;

        Ok(transaction.finish().await?)
    }

    pub async fn catalogue_opening(
        &self,
        within: Option<&mut Transaction>,
        scene: &str,
        project: &str,
        opening: &str,
        at: OffsetDateTime,
    ) -> Result<(), ScenesError> {
        let mut transaction = self.transactions.join(within).await?;
        self.catalog
            .note_opening(
                &mut transaction,
                scene.parse()?,
                &ProjectLink::from(project),
                opening,
                at,
            )
            .await?;

        Ok(transaction.finish().await?)
    }

    pub async fn catalogue_deleted(
        &self,
        within: Option<&mut Transaction>,
        scene: &str,
        at: OffsetDateTime,
    ) -> Result<(), ScenesError> {
        let mut transaction = self.transactions.join(within).await?;
        self.catalog
            .forget(&mut transaction, scene.parse()?, at)
            .await?;

        Ok(transaction.finish().await?)
    }

    pub async fn lay_project_tombstone(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        at: OffsetDateTime,
    ) -> Result<(), ScenesError> {
        let mut transaction = self.transactions.join(within).await?;
        self.catalog
            .lay_project_tombstone(&mut transaction, &ProjectLink::from(project), at)
            .await?;

        Ok(transaction.finish().await?)
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, ScenesError> {
        let mut transaction = self.transactions.begin().await?;
        let forgotten = self
            .catalog
            .forget_tombstones_laid_before(&mut transaction, before, at_most)
            .await?;
        transaction.finish().await?;

        Ok(forgotten)
    }

    async fn claimed(
        &self,
        project: &ProjectLink,
        restyling: &QuotationRestyling,
        at_most: usize,
    ) -> Result<Claimed, ScenesError> {
        let total = match restyling.after {
            Some(_) => 0,
            None => self.scenes_in(&restyling.project, at_most).await?,
        };
        let after = restyling.after.as_deref();

        let mut transaction = self.transactions.begin().await?;
        let claimed = if self
            .tasks
            .begin(&mut transaction, project, restyling.change, total, after)
            .await?
        {
            Claimed::Page
        } else {
            match self.tasks.standing(&mut transaction, project).await? {
                Some(standing) if standing.change > restyling.change => Claimed::Superseded,
                Some(standing) if standing.finished => Claimed::Passed(None),
                Some(standing) if standing.cursor.as_deref() == after => Claimed::Page,
                Some(standing) => {
                    Claimed::Passed(standing.cursor.as_deref().map(str::parse).transpose()?)
                }
                None => Claimed::Page,
            }
        };
        transaction.finish().await?;

        Ok(claimed)
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

enum Claimed {
    Page,
    Passed(Option<SceneId>),
    Superseded,
}
