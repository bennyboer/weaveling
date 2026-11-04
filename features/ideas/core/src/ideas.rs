use std::sync::Arc;

use authorizing::{AccessWatch, Act, Authorizer};
use clock::Clock;
use eventsourcing::{Agent, AggregateId, EventStore, ServiceError, Standing, Version};
use time::OffsetDateTime;
use transacting::{Transaction, Transactions};

use crate::catalog::{IdeaCatalog, IdeaSummary};
use crate::error::IdeasError;
use crate::id::IdeaId;
use crate::idea::{Idea, IdeaError, IdeaEvent, KIND, ProjectLink};
use crate::service::IdeaService;

#[derive(Clone)]
pub struct IdeasCore {
    clock: Arc<dyn Clock>,
    service: IdeaService,
    transactions: Arc<dyn Transactions>,
    catalog: Arc<dyn IdeaCatalog>,
    authorizer: Arc<dyn Authorizer>,
}

impl IdeasCore {
    pub fn new(
        store: Arc<dyn EventStore<IdeaEvent>>,
        transactions: Arc<dyn Transactions>,
        catalog: Arc<dyn IdeaCatalog>,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            service: IdeaService::new(store, clock.clone()),
            transactions,
            clock,
            catalog,
            authorizer,
        }
    }

    pub async fn list(&self, project: &str, agent: &Agent) -> Result<Vec<IdeaSummary>, IdeasError> {
        let project = ProjectLink::from(project);
        self.authorized(agent, Act::Read, &project, || IdeasError::ProjectNotThere)
            .await?;

        Ok(self.catalog.in_project(&project).await?)
    }

    pub async fn capture(
        &self,
        project: &str,
        title: &str,
        agent: &Agent,
    ) -> Result<(IdeaId, Standing<Idea>), IdeasError> {
        let project = ProjectLink::from(project);
        self.authorized(agent, Act::Write, &project, || IdeasError::ProjectNotThere)
            .await?;
        let mut transaction = self.transactions.begin().await?;
        if self
            .catalog
            .has_project_tombstone_within(&mut transaction, &project)
            .await?
        {
            return Err(IdeasError::ProjectNotThere);
        }
        let id = self
            .service
            .capture(&mut transaction, &project, title, agent)
            .await?;
        transaction.finish().await?;

        Ok((id, self.service.latest(&id).await?))
    }

    pub async fn read(&self, id: &str, agent: &Agent) -> Result<Standing<Idea>, IdeasError> {
        let id: IdeaId = id.parse()?;
        let standing = self.service.latest(&id).await?;
        self.authorized(agent, Act::Read, standing.state.project(), || {
            not_found(&id)
        })
        .await?;

        Ok(standing)
    }

    pub async fn retitle(
        &self,
        id: &str,
        title: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Standing<Idea>, IdeasError> {
        let id = self.authorized_to_write(id, agent).await?;
        transacting::retrying!(
            &*self.transactions,
            eventsourcing::tries_for(expected),
            |transaction| {
                self.service
                    .retitle(transaction, &id, title, expected, agent)
                    .await
            }
        )?;

        self.service.latest(&id).await
    }

    pub async fn discard(
        &self,
        id: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, IdeasError> {
        let id = self.authorized_to_write(id, agent).await?;

        transacting::retrying!(
            &*self.transactions,
            eventsourcing::tries_for(expected),
            |transaction| {
                self.service
                    .discard(transaction, &id, expected, agent)
                    .await
            }
        )
    }

    pub async fn may_follow(&self, project: &str, agent: &Agent) -> Result<bool, IdeasError> {
        Ok(self.authorizer.may(agent, Act::Read, project).await?)
    }

    pub fn watch_access(&self, project: &str) -> AccessWatch {
        self.authorizer.watch_access(project)
    }

    pub async fn project_of(&self, id: &str) -> Result<ProjectLink, IdeasError> {
        let id: IdeaId = id.parse()?;

        Ok(self.service.latest(&id).await?.state.project().clone())
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, IdeasError> {
        let mut transaction = self.transactions.begin().await?;
        let forgotten = self
            .catalog
            .forget_tombstones_laid_before(&mut transaction, before, at_most)
            .await?;
        transaction.finish().await?;

        Ok(forgotten)
    }

    pub async fn catalogue(
        &self,
        within: Option<&mut Transaction>,
        id: &IdeaId,
    ) -> Result<(), IdeasError> {
        let standing = self.service.latest(id).await?;

        let mut transaction = self.transactions.join(within).await?;
        if standing.state.is_discarded() {
            self.catalog
                .forget(&mut transaction, id, self.clock.now())
                .await?;
        } else if self
            .catalog
            .has_project_tombstone_within(&mut transaction, standing.state.project())
            .await?
        {
            self.service
                .discard(&mut transaction, id, None, &Agent::System)
                .await?;
        } else {
            self.catalog
                .remember(
                    &mut transaction,
                    &IdeaSummary::of(*id, standing.version, &standing.state),
                )
                .await?;
        }
        transaction.finish().await?;

        Ok(())
    }

    pub async fn lay_project_tombstone(
        &self,
        project: &str,
        at: OffsetDateTime,
    ) -> Result<(), IdeasError> {
        let mut transaction = self.transactions.begin().await?;
        self.catalog
            .lay_project_tombstone(&mut transaction, &ProjectLink::from(project), at)
            .await?;

        Ok(transaction.finish().await?)
    }

    pub async fn sweep(
        &self,
        project: &str,
        after: Option<&str>,
        at_most: usize,
    ) -> Result<Vec<IdeaId>, IdeasError> {
        let after = after.map(str::parse).transpose()?;
        let batch = self
            .catalog
            .in_project_after(&ProjectLink::from(project), after, at_most)
            .await?;

        for summary in &batch {
            match self
                .discard(&summary.id.to_string(), None, &Agent::System)
                .await
            {
                Ok(_) | Err(IdeasError::Events(ServiceError::Refused(IdeaError::Discarded))) => {}
                Err(refused) => return Err(refused),
            }
        }

        Ok(batch.iter().map(|summary| summary.id).collect())
    }

    async fn authorized_to_write(&self, id: &str, agent: &Agent) -> Result<IdeaId, IdeasError> {
        let id: IdeaId = id.parse()?;
        let standing = self.service.latest(&id).await?;
        self.authorized(agent, Act::Write, standing.state.project(), || {
            not_found(&id)
        })
        .await?;

        Ok(id)
    }

    async fn authorized(
        &self,
        agent: &Agent,
        act: Act,
        project: &ProjectLink,
        otherwise: impl FnOnce() -> IdeasError,
    ) -> Result<(), IdeasError> {
        if self.authorizer.may(agent, act, project.as_str()).await? {
            Ok(())
        } else if self
            .authorizer
            .may(agent, Act::Read, project.as_str())
            .await?
        {
            Err(IdeasError::NotAnAuthor)
        } else {
            Err(otherwise())
        }
    }
}

fn not_found(id: &IdeaId) -> IdeasError {
    IdeasError::Events(ServiceError::NotFound {
        aggregate: AggregateId::from(id),
        kind: KIND,
    })
}
