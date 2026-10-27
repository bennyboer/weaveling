use std::sync::Arc;

use authorizing::{AccessWatch, Act, Authorizer, Readable};
use clock::Clock;
use eventsourcing::{Agent, EventStore, Standing, Version};

use crate::catalog::{ProjectCatalog, ProjectSummary};
use crate::error::ProjectsError;
use crate::id::ProjectId;
use crate::project::{Project, ProjectEvent};
use crate::service::{self, ProjectService, RawDetails};

#[derive(Clone)]
pub struct ProjectsCore {
    service: ProjectService,
    catalog: Arc<dyn ProjectCatalog>,
    authorizer: Arc<dyn Authorizer>,
}

impl ProjectsCore {
    pub fn new(
        store: Arc<dyn EventStore<ProjectEvent>>,
        catalog: Arc<dyn ProjectCatalog>,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            service: ProjectService::new(store, clock),
            catalog,
            authorizer,
        }
    }

    pub async fn list(&self, agent: &Agent) -> Result<Vec<ProjectSummary>, ProjectsError> {
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
    ) -> Result<(ProjectId, Standing<Project>), ProjectsError> {
        let id = self.service.start(name, details, agent).await?;

        Ok((id, self.service.latest(&id).await?))
    }

    pub async fn read(&self, id: &str, agent: &Agent) -> Result<Standing<Project>, ProjectsError> {
        let id: ProjectId = id.parse()?;
        self.authorized(agent, Act::Read, &id).await?;

        self.service.latest(&id).await
    }

    pub async fn rename(
        &self,
        id: &str,
        name: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Standing<Project>, ProjectsError> {
        let id = self.authorized_to_manage(id, agent).await?;
        self.service.rename(&id, name, expected, agent).await?;

        self.service.latest(&id).await
    }

    pub async fn change_details(
        &self,
        id: &str,
        details: &RawDetails<'_>,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Standing<Project>, ProjectsError> {
        let id = self.authorized_to_manage(id, agent).await?;
        self.service
            .change_details(&id, details, expected, agent)
            .await?;

        self.service.latest(&id).await
    }

    pub async fn delete(
        &self,
        id: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Version, ProjectsError> {
        let id = self.authorized_to_manage(id, agent).await?;

        self.service.delete(&id, expected, agent).await
    }

    pub async fn may_follow(&self, project: &str, agent: &Agent) -> Result<bool, ProjectsError> {
        Ok(self.authorizer.may(agent, Act::Read, project).await?)
    }

    pub fn watch_access(&self, project: &str) -> AccessWatch {
        self.authorizer.watch_access(project)
    }

    pub async fn catalogue(&self, id: &ProjectId) -> Result<(), ProjectsError> {
        let standing = self.service.latest_even_deleted(id).await?;

        if standing.state.is_deleted() {
            self.catalog.forget(id).await?;
        } else {
            self.catalog
                .remember(&ProjectSummary::of(*id, standing.version, &standing.state))
                .await?;
        }

        Ok(())
    }

    async fn authorized_to_manage(
        &self,
        id: &str,
        agent: &Agent,
    ) -> Result<ProjectId, ProjectsError> {
        let id: ProjectId = id.parse()?;
        self.authorized(agent, Act::Manage, &id).await?;

        Ok(id)
    }

    async fn authorized(
        &self,
        agent: &Agent,
        act: Act,
        project: &ProjectId,
    ) -> Result<(), ProjectsError> {
        let named = project.to_string();
        if self.authorizer.may(agent, act, &named).await? {
            Ok(())
        } else if self.authorizer.may(agent, Act::Read, &named).await? {
            Err(ProjectsError::NotAnOwner)
        } else {
            Err(service::not_found(project))
        }
    }
}
