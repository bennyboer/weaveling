use std::sync::Arc;

use authorizing::{AccessWatch, Act, Authorizer, Readable};
use clock::Clock;
use eventsourcing::{Agent, EventStore, ServiceError, Standing, Version};
use time::{Duration, OffsetDateTime};
use transacting::{Transaction, Transactions};
use trash::Trash;

use crate::catalog::{ProjectCatalog, ProjectSummary};
use crate::error::ProjectsError;
use crate::id::ProjectId;
use crate::project::{KIND, Project, ProjectError, ProjectEvent};
use crate::service::{self, ProjectService, RawDetails};

pub const KEPT_IN_THE_TRASH_FOR: Duration = Duration::days(30);

#[derive(Clone)]
pub struct ProjectsCore {
    clock: Arc<dyn Clock>,
    service: ProjectService,
    transactions: Arc<dyn Transactions>,
    catalog: Arc<dyn ProjectCatalog>,
    trash: Arc<dyn Trash>,
    authorizer: Arc<dyn Authorizer>,
}

impl ProjectsCore {
    pub fn new(
        store: Arc<dyn EventStore<ProjectEvent>>,
        transactions: Arc<dyn Transactions>,
        catalog: Arc<dyn ProjectCatalog>,
        trash: Arc<dyn Trash>,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            service: ProjectService::new(store, clock.clone()),
            transactions,
            clock,
            catalog,
            trash,
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
        let id = transacting::retrying!(&*self.transactions, |transaction| {
            self.service.start(transaction, name, details, agent).await
        })?;

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
        transacting::retrying!(
            &*self.transactions,
            eventsourcing::tries_for(expected),
            |transaction| {
                self.service
                    .rename(transaction, &id, name, expected, agent)
                    .await
            }
        )?;

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
        transacting::retrying!(
            &*self.transactions,
            eventsourcing::tries_for(expected),
            |transaction| {
                self.service
                    .change_details(transaction, &id, details, expected, agent)
                    .await
            }
        )?;

        self.service.latest(&id).await
    }

    pub async fn trash(
        &self,
        id: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<(), ProjectsError> {
        let id = self.authorized_to_manage(id, agent).await?;
        let until = self.clock.now() + KEPT_IN_THE_TRASH_FOR;

        transacting::retrying!(
            &*self.transactions,
            eventsourcing::tries_for(expected),
            |transaction| {
                match self
                    .service
                    .trash(transaction, &id, until, expected, agent)
                    .await
                {
                    Err(ProjectsError::Events(ServiceError::Refused(
                        ProjectError::AlreadyTrashed,
                    ))) => Ok(()),
                    Err(failure) => Err(failure),
                    Ok(_) => Ok(self
                        .trash
                        .put_into(transaction, KIND.as_str(), &id.to_string(), until)
                        .await?),
                }
            }
        )
    }

    pub async fn restore(&self, id: &str, agent: &Agent) -> Result<(), ProjectsError> {
        let id: ProjectId = id.parse()?;
        self.authorized(agent, Act::Restore, &id).await?;

        transacting::retrying!(&*self.transactions, |transaction| {
            self.service.restore(transaction, &id, agent).await?;
            self.trash
                .take_out(transaction, KIND.as_str(), &id.to_string())
                .await?;

            Ok(())
        })
    }

    pub async fn trash_list(&self, agent: &Agent) -> Result<Vec<ProjectSummary>, ProjectsError> {
        let mut theirs = Vec::new();
        for trashed in self.catalog.in_the_trash().await? {
            if self
                .authorizer
                .may(agent, Act::Restore, &trashed.id.to_string())
                .await?
            {
                theirs.push(trashed);
            }
        }

        Ok(theirs)
    }

    pub async fn purge(&self, project: &str) -> Result<(), ProjectsError> {
        let id: ProjectId = project.parse()?;

        transacting::retrying!(&*self.transactions, |transaction| {
            match self
                .service
                .delete(transaction, &id, None, &Agent::System)
                .await
            {
                Ok(_)
                | Err(ProjectsError::Events(ServiceError::Refused(
                    ProjectError::NotTrashed | ProjectError::Deleted,
                ))) => {}
                Err(failure) => return Err(failure),
            }
            self.trash
                .take_out(transaction, KIND.as_str(), &id.to_string())
                .await?;

            Ok(())
        })
    }

    pub async fn may_hear_of(&self, project: &str, agent: &Agent) -> Result<bool, ProjectsError> {
        Ok(self.authorizer.may(agent, Act::Read, project).await?
            || self.authorizer.may(agent, Act::Restore, project).await?)
    }

    pub fn watch_access(&self, project: &str) -> AccessWatch {
        self.authorizer.watch_access(project)
    }

    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, ProjectsError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            let forgotten = self
                .catalog
                .forget_tombstones_laid_before(transaction, before, at_most)
                .await?;

            Ok(forgotten)
        })
    }

    pub async fn catalogue(
        &self,
        within: Option<&mut Transaction>,
        id: &ProjectId,
    ) -> Result<(), ProjectsError> {
        let standing = self.service.latest_even_deleted(id).await?;

        transacting::joining!(&*self.transactions, within, |transaction| {
            if standing.state.is_deleted() {
                self.catalog
                    .forget(transaction, id, self.clock.now())
                    .await?;
            } else {
                self.catalog
                    .remember(
                        transaction,
                        &ProjectSummary::of(*id, standing.version, &standing.state),
                    )
                    .await?;
            }

            Ok(())
        })
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
        } else if act != Act::Read && self.authorizer.may(agent, Act::Restore, &named).await? {
            Err(ProjectsError::Events(ServiceError::Refused(
                ProjectError::Trashed,
            )))
        } else {
            Err(service::not_found(project))
        }
    }
}
