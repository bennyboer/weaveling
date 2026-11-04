use std::sync::Arc;

use authorizing::{AccessWatch, Act, Authorizer};
use clock::Clock;
use eventsourcing::{AccountId, Agent, EventStore, ServiceError, Version};
use transacting::{Transaction, Transactions};

use crate::catalog::{Membership, MembershipCatalog, Person, ProjectMembers};
use crate::error::MembersCoreError;
use crate::members::{Member, MembersCommand, MembersError, MembersEvent, ProjectLink, Role};
use crate::service::MembersService;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedMember {
    pub account: AccountId,
    pub role: Role,
    pub name: Option<String>,
    pub email: Option<String>,
}

#[derive(Clone)]
pub struct MembersCore {
    service: MembersService,
    transactions: Arc<dyn Transactions>,
    catalog: Arc<dyn MembershipCatalog>,
    authorizer: Arc<dyn Authorizer>,
}

impl MembersCore {
    pub fn new(
        store: Arc<dyn EventStore<MembersEvent>>,
        transactions: Arc<dyn Transactions>,
        catalog: Arc<dyn MembershipCatalog>,
        authorizer: Arc<dyn Authorizer>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            service: MembersService::new(store, clock),
            transactions,
            catalog,
            authorizer,
        }
    }

    pub async fn members(
        &self,
        project: &str,
        agent: &Agent,
    ) -> Result<Vec<ListedMember>, MembersCoreError> {
        let project = ProjectLink::from(project);
        self.authorized(agent, Act::Read, &project).await?;
        let members = self.service.latest(&project).await?.state.members();

        self.listed(&project, members, agent).await
    }

    pub async fn change_role(
        &self,
        project: &str,
        account: &str,
        to: Role,
        agent: &Agent,
    ) -> Result<Vec<ListedMember>, MembersCoreError> {
        let project = ProjectLink::from(project);
        self.authorized(agent, Act::Manage, &project).await?;
        let members = self
            .written(
                &project,
                MembersCommand::ChangeRole {
                    account: AccountId::from(account),
                    to,
                },
                agent,
            )
            .await?;

        self.listed(&project, members, agent).await
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
        within: Option<&mut Transaction>,
        project: &str,
        account: &str,
        role: Role,
    ) -> Result<(), MembersCoreError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            match self
                .service
                .carry_out(
                    transaction,
                    &ProjectLink::from(project),
                    MembersCommand::Join {
                        account: AccountId::from(account),
                        role,
                    },
                    &Agent::System,
                )
                .await
            {
                Err(MembersCoreError::Events(ServiceError::Refused(
                    MembersError::AlreadyMember | MembersError::Disbanded,
                ))) => {}
                joined => joined?,
            }

            Ok(())
        })
    }

    pub async fn disband(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
    ) -> Result<(), MembersCoreError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.service
                .disband(transaction, &ProjectLink::from(project), &Agent::System)
                .await?;

            Ok(())
        })
    }

    pub async fn found(
        &self,
        within: Option<&mut Transaction>,
        project: &str,
        owner: &str,
    ) -> Result<(), MembersCoreError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.service
                .found(
                    transaction,
                    &ProjectLink::from(project),
                    AccountId::from(owner),
                    &Agent::System,
                )
                .await?;

            Ok(())
        })
    }

    pub async fn remember_person(
        &self,
        within: Option<&mut Transaction>,
        person: &Person,
    ) -> Result<(), MembersCoreError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog.remember_person(transaction, person).await?;

            Ok(())
        })
    }

    pub async fn catalogue(
        &self,
        within: Option<&mut Transaction>,
        project: &ProjectLink,
    ) -> Result<(), MembersCoreError> {
        let standing = self.service.latest(project).await?;

        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .remember(
                    transaction,
                    &ProjectMembers::of(project.clone(), standing.version, &standing.state),
                )
                .await?;
            let authorizer = self.authorizer.clone();
            let changed = project.as_str().to_owned();
            transaction.after_commit(async move { authorizer.access_changed(&changed) });

            Ok(())
        })
    }

    pub async fn remember_trash(
        &self,
        within: Option<&mut Transaction>,
        project: &ProjectLink,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), MembersCoreError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .remember_trash(transaction, project, version, in_the_trash)
                .await?;
            let authorizer = self.authorizer.clone();
            let changed = project.as_str().to_owned();
            transaction.after_commit(async move { authorizer.access_changed(&changed) });

            Ok(())
        })
    }

    pub async fn forget_trash(
        &self,
        within: Option<&mut Transaction>,
        project: &ProjectLink,
    ) -> Result<(), MembersCoreError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog.forget_trash(transaction, project).await?;

            Ok(())
        })
    }

    pub async fn memberships(&self, agent: &Agent) -> Result<Vec<Membership>, MembersCoreError> {
        match agent {
            Agent::Account(account) => Ok(self.catalog.memberships_of(account).await?),
            _ => Ok(Vec::new()),
        }
    }

    pub async fn may_follow(&self, agent: &Agent, project: &str) -> Result<bool, MembersCoreError> {
        Ok(self.authorizer.may(agent, Act::Read, project).await?)
    }

    pub fn watch_access(&self, project: &str) -> AccessWatch {
        self.authorizer.watch_access(project)
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
        transacting::retrying!(&*self.transactions, |transaction| {
            self.service
                .carry_out(transaction, project, command.clone(), agent)
                .await
        })?;

        Ok(self.service.latest(project).await?.state.members())
    }

    async fn listed(
        &self,
        project: &ProjectLink,
        members: Vec<Member>,
        agent: &Agent,
    ) -> Result<Vec<ListedMember>, MembersCoreError> {
        let accounts: Vec<AccountId> = members
            .iter()
            .map(|member| member.account.clone())
            .collect();
        let people = self.catalog.people(&accounts).await?;
        let owning = self
            .authorizer
            .may(agent, Act::Manage, project.as_str())
            .await?;

        Ok(members
            .into_iter()
            .map(|member| {
                let person = people
                    .iter()
                    .find(|person| person.account == member.account);

                ListedMember {
                    name: person.map(|person| person.name.clone()),
                    email: person.filter(|_| owning).map(|person| person.email.clone()),
                    account: member.account,
                    role: member.role,
                }
            })
            .collect())
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
