use std::sync::Arc;

use clock::Clock;
use eventsourcing::{
    Agent, AggregateId, EventSourcingService, EventStore, ServiceError, Standing, Version,
};

use crate::details::{BookLanguage, ProjectDescription, ProjectDetails, QuotationMarks};
use crate::error::ProjectsError;
use crate::id::ProjectId;
use crate::name::ProjectName;
use crate::project::{KIND, Project, ProjectCommand, ProjectEvent};

#[derive(Clone)]
pub struct ProjectService {
    events: Arc<EventSourcingService<Project>>,
    clock: Arc<dyn Clock>,
}

impl ProjectService {
    pub fn new(store: Arc<dyn EventStore<ProjectEvent>>, clock: Arc<dyn Clock>) -> Self {
        Self {
            events: Arc::new(EventSourcingService::new(store, clock.clone())),
            clock,
        }
    }

    pub async fn start(
        &self,
        name: &str,
        details: &RawDetails<'_>,
        agent: &Agent,
    ) -> Result<ProjectId, ProjectsError> {
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

    pub async fn latest(&self, id: &ProjectId) -> Result<Standing<Project>, ProjectsError> {
        let key = AggregateId::from(id);
        let standing = self.events.latest(&key).await?;

        if standing.state.is_deleted() {
            return Err(not_found(id));
        }

        Ok(standing)
    }

    pub async fn latest_even_deleted(
        &self,
        id: &ProjectId,
    ) -> Result<Standing<Project>, ProjectsError> {
        Ok(self.events.latest(&AggregateId::from(id)).await?)
    }

    pub async fn rename(
        &self,
        id: &ProjectId,
        name: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, ProjectsError> {
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
        id: &ProjectId,
        details: &RawDetails<'_>,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, ProjectsError> {
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
        id: &ProjectId,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, ProjectsError> {
        self.carry_out(id, ProjectCommand::Delete, expected, agent)
            .await
    }

    async fn carry_out(
        &self,
        id: &ProjectId,
        command: ProjectCommand,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, ProjectsError> {
        let key = AggregateId::from(id);

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

pub fn not_found(id: &ProjectId) -> ProjectsError {
    ProjectsError::Events(ServiceError::NotFound {
        aggregate: AggregateId::from(id),
        kind: KIND,
    })
}

#[derive(Default)]
pub struct RawDetails<'a> {
    pub language: Option<&'a str>,
    pub quotation: Option<&'a str>,
    pub description: &'a str,
}

impl RawDetails<'_> {
    fn checked(&self) -> Result<ProjectDetails, ProjectsError> {
        Ok(ProjectDetails {
            language: chosen(self.language).map(BookLanguage::new).transpose()?,
            quotation: chosen(self.quotation)
                .map(QuotationMarks::new)
                .transpose()?,
            description: ProjectDescription::new(self.description)
                .map_err(ProjectsError::InvalidDescription)?,
        })
    }
}

fn chosen(raw: Option<&str>) -> Option<&str> {
    raw.filter(|chosen| !chosen.trim().is_empty())
}
