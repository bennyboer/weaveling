use std::sync::Arc;

use authorizing::{Act, Authorizer};
use clock::Clock;
use eventsourcing::{AccountId, Agent, EventStore, ServiceError};

use crate::catalog::{MembershipCatalog, ProjectMembers};
use crate::error::MembersCoreError;
use crate::members::{Member, MembersCommand, MembersError, MembersEvent, ProjectLink, Role};
use crate::service::MembersService;

#[derive(Clone)]
pub struct MembersCore {
    service: MembersService,
    catalog: Arc<dyn MembershipCatalog>,
    authorizer: Arc<dyn Authorizer>,
}

impl MembersCore {
    pub fn new(
        store: Arc<dyn EventStore<MembersEvent>>,
        catalog: Arc<dyn MembershipCatalog>,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            service: MembersService::new(store, clock),
            catalog,
            authorizer,
        }
    }

    pub async fn members(
        &self,
        project: &str,
        agent: &Agent,
    ) -> Result<Vec<Member>, MembersCoreError> {
        let project = ProjectLink::from(project);
        self.authorized(agent, Act::Read, &project).await?;

        Ok(self.service.latest(&project).await?.state.members())
    }

    pub async fn change_role(
        &self,
        project: &str,
        account: &str,
        to: Role,
        agent: &Agent,
    ) -> Result<Vec<Member>, MembersCoreError> {
        let project = ProjectLink::from(project);
        self.authorized(agent, Act::Manage, &project).await?;

        self.written(
            &project,
            MembersCommand::ChangeRole {
                account: AccountId::from(account),
                to,
            },
            agent,
        )
        .await
    }

    pub async fn remove(
        &self,
        project: &str,
        account: &str,
        agent: &Agent,
    ) -> Result<Vec<Member>, MembersCoreError> {
        let project = ProjectLink::from(project);
        self.authorized(agent, Act::Manage, &project).await?;

        self.written(
            &project,
            MembersCommand::Remove {
                account: AccountId::from(account),
            },
            agent,
        )
        .await
    }

    pub async fn leave(&self, project: &str, agent: &Agent) -> Result<(), MembersCoreError> {
        let Agent::Account(account) = agent else {
            return Err(MembersCoreError::Events(ServiceError::Refused(
                MembersError::NotMember,
            )));
        };
        let project = ProjectLink::from(project);
        let members = match self.service.latest(&project).await {
            Ok(standing) => standing.state,
            Err(MembersCoreError::Events(ServiceError::NotFound { .. })) => {
                return Err(MembersCoreError::ProjectNotThere);
            }
            Err(failed) => return Err(failed),
        };
        if members.role_of(account).is_none() {
            return Err(MembersCoreError::ProjectNotThere);
        }

        self.written(
            &project,
            MembersCommand::Leave {
                account: account.clone(),
            },
            agent,
        )
        .await?;

        Ok(())
    }

    pub async fn join(
        &self,
        project: &str,
        account: &str,
        role: Role,
    ) -> Result<(), MembersCoreError> {
        match self
            .written(
                &ProjectLink::from(project),
                MembersCommand::Join {
                    account: AccountId::from(account),
                    role,
                },
                &Agent::System,
            )
            .await
        {
            Err(MembersCoreError::Events(ServiceError::Refused(MembersError::AlreadyMember))) => {
                Ok(())
            }
            joined => joined.map(|_| ()),
        }
    }

    pub async fn found(&self, project: &str, owner: &str) -> Result<(), MembersCoreError> {
        self.service
            .found(
                &ProjectLink::from(project),
                AccountId::from(owner),
                &Agent::System,
            )
            .await
    }

    pub async fn catalogue(&self, project: &ProjectLink) -> Result<(), MembersCoreError> {
        let standing = self.service.latest(project).await?;

        self.catalog
            .remember(&ProjectMembers::of(
                project.clone(),
                standing.version,
                &standing.state,
            ))
            .await?;
        self.authorizer.access_changed(project.as_str());

        Ok(())
    }

    pub fn may_hear(&self, agent: &Agent, about: &str) -> bool {
        matches!(agent, Agent::Account(follower) if follower.as_str() == about)
    }

    async fn written(
        &self,
        project: &ProjectLink,
        command: MembersCommand,
        agent: &Agent,
    ) -> Result<Vec<Member>, MembersCoreError> {
        self.service.carry_out(project, command, agent).await?;

        Ok(self.service.latest(project).await?.state.members())
    }

    async fn authorized(
        &self,
        agent: &Agent,
        act: Act,
        project: &ProjectLink,
    ) -> Result<(), MembersCoreError> {
        if self.authorizer.may(agent, act, project.as_str()).await? {
            Ok(())
        } else if self
            .authorizer
            .may(agent, Act::Read, project.as_str())
            .await?
        {
            Err(MembersCoreError::NotAnOwner)
        } else {
            Err(MembersCoreError::ProjectNotThere)
        }
    }
}
