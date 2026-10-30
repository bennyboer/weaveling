use std::sync::Arc;

use authorizing::{AccessWatch, Act, Authorizer};
use clock::Clock;
use eventsourcing::{Agent, EventStore, ServiceError, Standing, Version};
use registry::Registry;
use time::OffsetDateTime;

use crate::catalog::{OutlineCatalog, OutlineSummary};
use crate::error::OutlinesError;
use crate::id::{OutlineId, SectionId};
use crate::outline::{
    Attachment, Outline, OutlineCommand, OutlineError, OutlineEvent, ProjectLink,
};
use crate::service::{self, Added, Changed, Open, OutlineService};
use crate::title::SectionTitle;

#[derive(Clone)]
pub struct OutlinesCore {
    service: OutlineService,
    catalog: Arc<dyn OutlineCatalog>,
    authorizer: Arc<dyn Authorizer>,
}

impl OutlinesCore {
    pub fn new(
        store: Arc<dyn EventStore<OutlineEvent>>,
        catalog: Arc<dyn OutlineCatalog>,
        registry: Arc<dyn Registry>,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            service: OutlineService::new(store, registry, clock),
            catalog,
            authorizer,
        }
    }

    pub async fn open(&self, project: &str, agent: &Agent) -> Result<Open, OutlinesError> {
        let project = ProjectLink::from(project);
        self.authorized(agent, Act::Read, &project, || {
            OutlinesError::ProjectNotThere
        })
        .await?;

        self.service.open(&project, agent).await
    }

    pub async fn read(
        &self,
        outline: &str,
        agent: &Agent,
    ) -> Result<Standing<Outline>, OutlinesError> {
        let outline: OutlineId = outline.parse()?;
        let standing = self.service.latest(&outline).await?;
        self.authorized(agent, Act::Read, standing.state.project(), || {
            service::not_found(&outline)
        })
        .await?;

        Ok(standing)
    }

    pub async fn watch_access(&self, outline: &str) -> Result<AccessWatch, OutlinesError> {
        let outline: OutlineId = outline.parse()?;
        let standing = self.service.latest(&outline).await?;

        Ok(self
            .authorizer
            .watch_access(standing.state.project().as_str()))
    }

    pub async fn add(
        &self,
        outline: &str,
        under: Option<SectionId>,
        after: Option<SectionId>,
        title: SectionTitle,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Added, OutlinesError> {
        let section = self.service.new_section();
        let changed = self
            .written(
                outline,
                OutlineCommand::Add {
                    section,
                    under,
                    after,
                    title,
                },
                expected,
                agent,
            )
            .await?;

        Ok(Added {
            section,
            version: changed.version,
            standing: changed.standing,
        })
    }

    pub async fn retitle(
        &self,
        outline: &str,
        section: SectionId,
        title: SectionTitle,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Changed, OutlinesError> {
        self.written(
            outline,
            OutlineCommand::Retitle { section, title },
            expected,
            agent,
        )
        .await
    }

    pub async fn place(
        &self,
        outline: &str,
        section: SectionId,
        under: Option<SectionId>,
        after: Option<SectionId>,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Changed, OutlinesError> {
        self.written(
            outline,
            OutlineCommand::Move {
                section,
                under,
                after,
            },
            expected,
            agent,
        )
        .await
    }

    pub async fn promote(
        &self,
        outline: &str,
        section: SectionId,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Changed, OutlinesError> {
        self.written(
            outline,
            OutlineCommand::Promote { section },
            expected,
            agent,
        )
        .await
    }

    pub async fn demote(
        &self,
        outline: &str,
        section: SectionId,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Changed, OutlinesError> {
        self.written(outline, OutlineCommand::Demote { section }, expected, agent)
            .await
    }

    pub async fn remove(
        &self,
        outline: &str,
        section: SectionId,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Changed, OutlinesError> {
        self.written(outline, OutlineCommand::Remove { section }, expected, agent)
            .await
    }

    pub async fn attach(
        &self,
        outline: &str,
        attachment: Attachment,
        to: SectionId,
        after: Option<Attachment>,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Changed, OutlinesError> {
        self.written(
            outline,
            OutlineCommand::Attach {
                attachment,
                to,
                after,
            },
            expected,
            agent,
        )
        .await
    }

    pub async fn detach(
        &self,
        outline: &str,
        attachment: Attachment,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Changed, OutlinesError> {
        self.written(
            outline,
            OutlineCommand::Detach { attachment },
            expected,
            agent,
        )
        .await
    }

    pub async fn catalogue(&self, outline: &OutlineId) -> Result<(), OutlinesError> {
        let standing = self.service.latest(outline).await?;

        if standing.state.is_discarded() {
            self.catalog.forget(outline).await?;
        } else {
            self.catalog
                .remember(&OutlineSummary::of(*outline, &standing.state))
                .await?;
        }

        Ok(())
    }

    pub async fn index_attachments(&self, outline: &OutlineId) -> Result<(), OutlinesError> {
        let standing = self.service.latest(outline).await?;
        if standing.state.is_discarded() {
            self.catalog.holds(*outline, &[]).await?;
            return Ok(());
        }

        self.catalog
            .holds(*outline, &standing.state.attachments())
            .await?;

        for gone in self.catalog.held_though_tombstoned(*outline).await? {
            self.let_go(*outline, &gone).await?;
        }

        Ok(())
    }

    pub async fn let_go_everywhere(
        &self,
        gone: &Attachment,
        at: OffsetDateTime,
    ) -> Result<(), OutlinesError> {
        self.catalog.lay_tombstone(gone, at).await?;

        for outline in self.catalog.outlines_holding(gone).await? {
            self.let_go(outline, gone).await?;
        }

        Ok(())
    }

    pub async fn sweep(&self, project: &str) -> Result<(), OutlinesError> {
        let Some(outline) = self.service.outline_of(&ProjectLink::from(project)).await? else {
            return Ok(());
        };

        match self.as_the_system(&outline, OutlineCommand::Discard).await {
            Ok(()) | Err(OutlinesError::Events(ServiceError::Refused(OutlineError::Discarded))) => {
                Ok(())
            }
            Err(refused) => Err(refused),
        }
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, OutlinesError> {
        Ok(self
            .catalog
            .forget_tombstones_laid_before(before, at_most)
            .await?)
    }

    async fn let_go(&self, outline: OutlineId, gone: &Attachment) -> Result<(), OutlinesError> {
        match self
            .as_the_system(
                &outline,
                OutlineCommand::Detach {
                    attachment: gone.clone(),
                },
            )
            .await
        {
            Ok(())
            | Err(OutlinesError::Events(ServiceError::Refused(
                OutlineError::NotAttached | OutlineError::Discarded,
            ))) => Ok(()),
            Err(refused) => Err(refused),
        }
    }

    async fn as_the_system(
        &self,
        outline: &OutlineId,
        command: OutlineCommand,
    ) -> Result<(), OutlinesError> {
        self.authorized_to_write(outline, &Agent::System).await?;
        self.service
            .carry_out(outline, command, None, &Agent::System)
            .await?;

        Ok(())
    }

    async fn written(
        &self,
        outline: &str,
        command: OutlineCommand,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Changed, OutlinesError> {
        let outline: OutlineId = outline.parse()?;
        self.authorized_to_write(&outline, agent).await?;

        self.service
            .carry_out(&outline, command, expected, agent)
            .await
    }

    async fn authorized_to_write(
        &self,
        outline: &OutlineId,
        agent: &Agent,
    ) -> Result<(), OutlinesError> {
        let standing = self.service.latest(outline).await?;

        self.authorized(agent, Act::Write, standing.state.project(), || {
            service::not_found(outline)
        })
        .await
    }

    async fn authorized(
        &self,
        agent: &Agent,
        act: Act,
        project: &ProjectLink,
        otherwise: impl FnOnce() -> OutlinesError,
    ) -> Result<(), OutlinesError> {
        if self.authorizer.may(agent, act, project.as_str()).await? {
            Ok(())
        } else if self
            .authorizer
            .may(agent, Act::Read, project.as_str())
            .await?
        {
            Err(OutlinesError::NotAnAuthor)
        } else {
            Err(otherwise())
        }
    }
}
