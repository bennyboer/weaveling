use std::sync::Arc;

use authorizing::{AccessWatch, Act, Authorizer};
use clock::Clock;
use eventsourcing::{Agent, EventStore, ServiceError, Standing, Version};
use registry::Registry;
use time::OffsetDateTime;
use transacting::{Transaction, Transactions};

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
    clock: Arc<dyn Clock>,
    service: OutlineService,
    transactions: Arc<dyn Transactions>,
    catalog: Arc<dyn OutlineCatalog>,
    authorizer: Arc<dyn Authorizer>,
}

impl OutlinesCore {
    pub fn new(
        store: Arc<dyn EventStore<OutlineEvent>>,
        transactions: Arc<dyn Transactions>,
        catalog: Arc<dyn OutlineCatalog>,
        registry: Arc<dyn Registry>,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            service: OutlineService::new(store, registry, clock.clone()),
            transactions,
            clock,
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
        let id = transacting::retrying!(&*self.transactions, |transaction| {
            if self
                .catalog
                .has_project_tombstone_within(transaction, &project)
                .await?
            {
                return Err(OutlinesError::ProjectNotThere);
            }
            self.service.claim(transaction, &project, agent).await
        })?;

        Ok(Open {
            id,
            standing: self.service.latest(&id).await?,
        })
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
            since: changed.since,
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

    pub async fn catalogue(
        &self,
        within: Option<&mut Transaction>,
        outline: &OutlineId,
    ) -> Result<(), OutlinesError> {
        let standing = self.service.latest(outline).await?;

        transacting::joining!(&*self.transactions, within, |transaction| {
            if standing.state.is_discarded() {
                self.catalog
                    .forget(transaction, outline, self.clock.now())
                    .await?;
            } else {
                self.catalog
                    .remember(transaction, &OutlineSummary::of(*outline, &standing.state))
                    .await?;
            }

            Ok(())
        })
    }

    pub async fn index_attachments(
        &self,
        within: Option<&mut Transaction>,
        outline: &OutlineId,
    ) -> Result<(), OutlinesError> {
        let standing = self.service.latest(outline).await?;
        let holding = if standing.state.is_discarded() {
            Vec::new()
        } else {
            standing.state.attachments()
        };

        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .holds(transaction, *outline, standing.version.count(), &holding)
                .await?;

            Ok(())
        })
    }

    pub async fn lay_tombstone(
        &self,
        within: Option<&mut Transaction>,
        gone: &Attachment,
        at: OffsetDateTime,
    ) -> Result<(), OutlinesError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog.lay_tombstone(transaction, gone, at).await?;

            Ok(())
        })
    }

    pub async fn let_go_everywhere(
        &self,
        within: Option<&mut Transaction>,
        gone: &Attachment,
    ) -> Result<(), OutlinesError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            for outline in self.catalog.outlines_holding(gone).await? {
                self.let_go(transaction, outline, gone).await?;
            }

            Ok(())
        })
    }

    pub async fn let_go_of_the_gone(
        &self,
        within: Option<&mut Transaction>,
        outline: &OutlineId,
    ) -> Result<(), OutlinesError> {
        let still_attached = self.service.latest(outline).await?.state.attachments();
        let Some(gone) = self
            .catalog
            .held_though_tombstoned(*outline)
            .await?
            .into_iter()
            .find(|attachment| still_attached.contains(attachment))
        else {
            return Ok(());
        };

        transacting::joining!(&*self.transactions, within, |transaction| {
            self.let_go(transaction, *outline, &gone).await?;

            Ok(())
        })
    }

    pub async fn sweep(&self, project: &str) -> Result<(), OutlinesError> {
        let Some(outline) = self.service.outline_of(&ProjectLink::from(project)).await? else {
            return Ok(());
        };

        self.discarded(&outline).await
    }

    async fn discarded(&self, outline: &OutlineId) -> Result<(), OutlinesError> {
        match self.as_the_system(outline, OutlineCommand::Discard).await {
            Ok(())
            | Err(OutlinesError::Events(ServiceError::Refused(OutlineError::Discarded)))
            | Err(OutlinesError::Events(ServiceError::NotFound { .. })) => Ok(()),
            Err(refused) => Err(refused),
        }
    }

    pub async fn lay_project_tombstone(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        at: OffsetDateTime,
    ) -> Result<(), OutlinesError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .lay_project_tombstone(transaction, &ProjectLink::from(project), at)
                .await?;

            Ok(())
        })
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, OutlinesError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            let forgotten = self
                .catalog
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?;

            Ok(forgotten)
        })
    }

    async fn let_go(
        &self,
        transaction: &mut Transaction,
        outline: OutlineId,
        gone: &Attachment,
    ) -> Result<(), OutlinesError> {
        self.authorized_to_write(&outline, &Agent::System).await?;
        match self
            .service
            .carry_out(
                transaction,
                &outline,
                OutlineCommand::Detach {
                    attachment: gone.clone(),
                },
                None,
                &Agent::System,
            )
            .await
        {
            Ok(_)
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
        transacting::retrying!(&*self.transactions, |transaction| {
            self.service
                .carry_out(transaction, outline, command.clone(), None, &Agent::System)
                .await
        })?;

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
        let landed = transacting::retrying!(
            &*self.transactions,
            eventsourcing::tries_for(expected),
            |transaction| {
                self.service
                    .carry_out(transaction, &outline, command.clone(), expected, agent)
                    .await
            }
        )?;

        Ok(Changed {
            since: Version::of(
                landed.version.count()
                    - u64::try_from(landed.events.len())
                        .expect("a command appends a handful of events"),
            ),
            version: landed.version,
            attachments_changed_in: service::attachments_changed_in(&landed),
            standing: self.service.latest(&outline).await?,
        })
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
