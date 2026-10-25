use std::sync::Arc;

use authorizing::{Authorizer, AuthorizerError, Readable};
use clock::Clock;
use eventsourcing::{
    Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing, Version,
};
use ids::InvalidId;
use thiserror::Error;

use crate::catalog::{CatalogError, ProjectCatalog, ProjectSummary};
use crate::details::{
    BookLanguage, InvalidBookLanguage, InvalidProjectDescription, InvalidQuotationMarks,
    ProjectDescription, ProjectDetails, QuotationMarks,
};
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
    InvalidLanguage(#[from] InvalidBookLanguage),
    #[error(transparent)]
    InvalidQuotation(#[from] InvalidQuotationMarks),
    #[error(transparent)]
    InvalidDescription(InvalidProjectDescription),
    #[error(transparent)]
    Events(#[from] ServiceError<ProjectError>),
    #[error(transparent)]
    Catalog(#[from] CatalogError),
    #[error(transparent)]
    Authorizer(#[from] AuthorizerError),
}

#[derive(Clone)]
pub struct ProjectService {
    events: Arc<EventSourcingService<Project>>,
    catalog: Arc<dyn ProjectCatalog>,
    authorizer: Arc<dyn Authorizer>,
    clock: Arc<dyn Clock>,
}

impl ProjectService {
    pub fn new(
        store: Arc<dyn EventStore<ProjectEvent>>,
        catalog: Arc<dyn ProjectCatalog>,
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

    pub async fn list(&self, agent: &Agent) -> Result<Vec<ProjectSummary>, ProjectServiceError> {
        match self.authorizer.readable_by(agent).await? {
            Readable::Everything => Ok(self.catalog.all().await?),
            Readable::Only(projects) => {
                let ids: Vec<ProjectId> = projects
                    .iter()
                    .filter_map(|project| project.parse().ok())
                    .collect();

                Ok(self.catalog.among(&ids).await?)
            }
        }
    }

    pub async fn start(
        &self,
        name: &str,
        details: &RawDetails<'_>,
        agent: &Agent,
    ) -> Result<ProjectId, ProjectServiceError> {
        let id = ProjectId::generate(self.clock.now());

        let _landed = self
            .events
            .begin(
                &AggregateId::from(&id),
                ProjectCommand::Start(ProjectName::new(name)?, details.checked()?),
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

    pub async fn change_details(
        &self,
        id: &str,
        details: &RawDetails<'_>,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, ProjectServiceError> {
        self.carry_out(
            id,
            ProjectCommand::ChangeDetails(details.checked()?),
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

#[derive(Default)]
pub struct RawDetails<'a> {
    pub language: Option<&'a str>,
    pub quotation: Option<&'a str>,
    pub description: &'a str,
}

impl RawDetails<'_> {
    fn checked(&self) -> Result<ProjectDetails, ProjectServiceError> {
        Ok(ProjectDetails {
            language: chosen(self.language).map(BookLanguage::new).transpose()?,
            quotation: chosen(self.quotation)
                .map(QuotationMarks::new)
                .transpose()?,
            description: ProjectDescription::new(self.description)
                .map_err(ProjectServiceError::InvalidDescription)?,
        })
    }
}

fn chosen(raw: Option<&str>) -> Option<&str> {
    raw.filter(|chosen| !chosen.trim().is_empty())
}
