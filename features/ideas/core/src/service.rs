use std::sync::Arc;

use authorizing::{Act, Authorizer, AuthorizerError};
use clock::Clock;
use eventsourcing::{
    Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing, Version,
};
use ids::InvalidId;
use thiserror::Error;

use crate::catalog::{CatalogError, IdeaCatalog, IdeaSummary};
use crate::id::IdeaId;
use crate::idea::{Idea, IdeaCommand, IdeaError, IdeaEvent, KIND, ProjectLink};
use crate::title::{IdeaTitle, InvalidIdeaTitle};

#[derive(Debug, Error)]
pub enum IdeaServiceError {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    InvalidTitle(#[from] InvalidIdeaTitle),
    #[error(transparent)]
    Events(#[from] ServiceError<IdeaError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Authorizer(#[from] AuthorizerError),
    #[error("there is no such project")]
    ProjectNotThere,
    #[error("only an author of the project may do this")]
    NotAnAuthor,
}

#[derive(Clone)]
pub struct IdeaService {
    events: Arc<EventSourcingService<Idea>>,
    catalog: Arc<dyn IdeaCatalog>,
    authorizer: Arc<dyn Authorizer>,
    clock: Arc<dyn Clock>,
}

impl IdeaService {
    pub fn new(
        store: Arc<dyn EventStore<IdeaEvent>>,
        catalog: Arc<dyn IdeaCatalog>,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(store, clock.clone())),
            catalog,
            authorizer,
            clock,
        }
    }

    pub async fn list(
        &self,
        project: &str,
        agent: &Agent,
    ) -> Result<Vec<IdeaSummary>, IdeaServiceError> {
        let project = ProjectLink::from(project);
        self.admitted(agent, Act::Read, &project, || {
            IdeaServiceError::ProjectNotThere
        })
        .await?;

        Ok(self.catalog.in_project(&project).await?)
    }

    pub async fn capture(
        &self,
        project: &str,
        title: &str,
        agent: &Agent,
    ) -> Result<IdeaId, IdeaServiceError> {
        let project = ProjectLink::from(project);
        self.admitted(agent, Act::Write, &project, || {
            IdeaServiceError::ProjectNotThere
        })
        .await?;
        let id = IdeaId::generate(self.clock.now());

        let _landed = self
            .events
            .begin(
                &AggregateId::from(&id),
                IdeaCommand::Capture {
                    project,
                    title: IdeaTitle::new(title)?,
                },
                agent,
            )
            .await?;

        Ok(id)
    }

    pub async fn get(&self, id: &str) -> Result<Standing<Idea>, IdeaServiceError> {
        let id: IdeaId = id.parse()?;

        Ok(self.events.latest(&AggregateId::from(&id)).await?)
    }

    pub async fn read(&self, id: &str, agent: &Agent) -> Result<Standing<Idea>, IdeaServiceError> {
        let idea: IdeaId = id.parse()?;
        let key = AggregateId::from(&idea);
        let standing = self.events.latest(&key).await?;
        self.admitted(agent, Act::Read, standing.state.project(), || {
            not_found(key)
        })
        .await?;

        Ok(standing)
    }

    async fn admitted(
        &self,
        agent: &Agent,
        act: Act,
        project: &ProjectLink,
        otherwise: impl FnOnce() -> IdeaServiceError,
    ) -> Result<(), IdeaServiceError> {
        if self.authorizer.may(agent, act, project.as_str()).await? {
            Ok(())
        } else if self
            .authorizer
            .may(agent, Act::Read, project.as_str())
            .await?
        {
            Err(IdeaServiceError::NotAnAuthor)
        } else {
            Err(otherwise())
        }
    }

    pub async fn retitle(
        &self,
        id: &str,
        title: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, IdeaServiceError> {
        self.carry_out(
            id,
            IdeaCommand::Retitle(IdeaTitle::new(title)?),
            expected,
            agent,
        )
        .await
    }

    pub async fn discard(
        &self,
        id: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, IdeaServiceError> {
        self.carry_out(id, IdeaCommand::Discard, expected, agent)
            .await
    }

    async fn carry_out(
        &self,
        id: &str,
        command: IdeaCommand,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, IdeaServiceError> {
        let id: IdeaId = id.parse()?;
        let key = AggregateId::from(&id);
        let standing = self.events.latest(&key).await?;
        self.admitted(agent, Act::Write, standing.state.project(), || {
            not_found(key.clone())
        })
        .await?;

        let landed = match expected {
            Some(expected) => {
                self.events
                    .execute_at(&key, expected, command, agent)
                    .await?
            }
            None => self.events.execute(&key, command, agent).await?,
        };

        Ok(landed.version)
    }
}

fn not_found(key: AggregateId) -> IdeaServiceError {
    IdeaServiceError::Events(ServiceError::NotFound {
        aggregate: key,
        kind: KIND,
    })
}
