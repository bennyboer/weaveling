use std::sync::Arc;

use authorizing::{AccessWatch, Act, Authorizer};
use clock::Clock;
use eventsourcing::{Agent, AggregateId, EventStore, ServiceError, Standing, Version};

use crate::catalog::{IdeaCatalog, IdeaSummary};
use crate::error::IdeasError;
use crate::id::IdeaId;
use crate::idea::{Idea, IdeaError, IdeaEvent, KIND, ProjectLink};
use crate::service::IdeaService;

#[derive(Clone)]
pub struct IdeasCore {
    service: IdeaService,
    catalog: Arc<dyn IdeaCatalog>,
    authorizer: Arc<dyn Authorizer>,
}

impl IdeasCore {
    pub fn new(
        store: Arc<dyn EventStore<IdeaEvent>>,
        catalog: Arc<dyn IdeaCatalog>,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            service: IdeaService::new(store, clock),
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
        let id = self.service.capture(&project, title, agent).await?;

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
        self.service.retitle(&id, title, expected, agent).await?;

        self.service.latest(&id).await
    }

    pub async fn discard(
        &self,
        id: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, IdeasError> {
        let id = self.authorized_to_write(id, agent).await?;

        self.service.discard(&id, expected, agent).await
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

    pub async fn catalogue(&self, id: &IdeaId) -> Result<(), IdeasError> {
        let standing = self.service.latest(id).await?;

        if standing.state.is_discarded() {
            self.catalog.forget(id).await?;
        } else {
            self.catalog
                .remember(&IdeaSummary::of(*id, standing.version, &standing.state))
                .await?;
        }

        Ok(())
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
