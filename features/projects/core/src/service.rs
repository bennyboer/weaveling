use std::sync::Arc;

use clock::Clock;
use eventsourcing::{
    Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing, Version,
};
use ids::InvalidId;
use thiserror::Error;

use crate::catalog::{CatalogError, ProjectCatalog, ProjectSummary};
use crate::id::ProjectId;
use crate::name::{InvalidProjectName, ProjectName};
use crate::project::{KIND, Project, ProjectCommand, ProjectError, ProjectEvent};

#[derive(Debug, Error)]
pub enum ProjectServiceError {
    #[error(transparent)]
    InvalidId(#[from] InvalidId),
    #[error(transparent)]
    InvalidName(#[from] InvalidProjectName),
    #[error(transparent)]
    Events(#[from] ServiceError<ProjectError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
}

#[derive(Clone)]
pub struct ProjectService {
    events: Arc<EventSourcingService<Project>>,
    catalog: Arc<dyn ProjectCatalog>,
    clock: Arc<dyn Clock>,
}

impl ProjectService {
    pub fn new(
        store: Arc<dyn EventStore<ProjectEvent>>,
        catalog: Arc<dyn ProjectCatalog>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(store, clock.clone())),
            catalog,
            clock,
        }
    }

    pub async fn list(&self) -> Result<Vec<ProjectSummary>, ProjectServiceError> {
        Ok(self.catalog.all().await?)
    }

    pub async fn start(&self, name: &str, agent: &Agent) -> Result<ProjectId, ProjectServiceError> {
        let id = ProjectId::generate(self.clock.now());

        let _landed = self
            .events
            .begin(
                &AggregateId::from(&id),
                ProjectCommand::Start(ProjectName::new(name)?),
                agent,
            )
            .await?;

        Ok(id)
    }

    pub async fn get(&self, id: &str) -> Result<Standing<Project>, ProjectServiceError> {
        let id: ProjectId = id.parse()?;
        let key = AggregateId::from(&id);
        let standing = self.events.latest(&key).await?;

        if standing.state.is_deleted() {
            return Err(ProjectServiceError::Events(ServiceError::NotFound {
                aggregate: key,
                kind: KIND,
            }));
        }

        Ok(standing)
    }

    pub async fn rename(
        &self,
        id: &str,
        name: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, ProjectServiceError> {
        self.carry_out(
            id,
            ProjectCommand::Rename(ProjectName::new(name)?),
            expected,
            agent,
        )
        .await
    }

    pub async fn delete(
        &self,
        id: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, ProjectServiceError> {
        self.carry_out(id, ProjectCommand::Delete, expected, agent)
            .await
    }

    async fn carry_out(
        &self,
        id: &str,
        command: ProjectCommand,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, ProjectServiceError> {
        let id: ProjectId = id.parse()?;
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
