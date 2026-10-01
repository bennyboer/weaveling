use std::sync::Arc;

use clock::Clock;
use eventsourcing::{
    Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing, Version,
};
use ids::InvalidId;
use thiserror::Error;

use crate::catalog::{CatalogError, IdeaCatalog, IdeaSummary};
use crate::id::IdeaId;
use crate::idea::{Idea, IdeaCommand, IdeaError, IdeaEvent, ProjectLink};
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
}

#[derive(Clone)]
pub struct IdeaService {
    events: Arc<EventSourcingService<Idea>>,
    catalog: Arc<dyn IdeaCatalog>,
    clock: Arc<dyn Clock>,
}

impl IdeaService {
    pub fn new(
        store: Arc<dyn EventStore<IdeaEvent>>,
        catalog: Arc<dyn IdeaCatalog>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(store, clock.clone())),
            catalog,
            clock,
        }
    }

    pub async fn list(&self, project: &str) -> Result<Vec<IdeaSummary>, IdeaServiceError> {
        Ok(self.catalog.in_project(&ProjectLink::from(project)).await?)
    }

    pub async fn capture(
        &self,
        project: &str,
        title: &str,
        agent: &Agent,
    ) -> Result<IdeaId, IdeaServiceError> {
        let id = IdeaId::generate(self.clock.now());

        let _landed = self
            .events
            .begin(
                &AggregateId::from(&id),
                IdeaCommand::Capture {
                    project: ProjectLink::from(project),
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
