use std::slice;
use std::sync::Arc;

use authorizing::{AccessWatch, Act, Authorizer};
use clock::Clock;
use eventsourcing::{AccountId, Agent, EventStore, ServiceError, Standing, Version};
use time::OffsetDateTime;
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
    pub leaving_until: Option<OffsetDateTime>,
}

#[derive(Clone)]
pub struct MembersCore {
    service: MembersService,
    transactions: Arc<dyn Transactions>,
    catalog: Arc<dyn MembershipCatalog>,
    authorizer: Arc<dyn Authorizer>,
    clock: Arc<dyn Clock>,
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
            service: MembersService::new(store, clock.clone()),
            transactions,
            catalog,
            authorizer,
            clock,
        }
    }

    pub async fn members(
        &self,
        project: &str,
        agent: &Agent,
    ) -> Result<Standing<Vec<ListedMember>>, MembersCoreError> {
        let project = ProjectLink::from(project);
        self.authorized(agent, Act::Read, &project).await?;
        let standing = self.service.latest(&project).await?;

        self.listed(
            &project,
            Standing {
                state: standing.state.members(),
                version: standing.version,
            },
            agent,
        )
        .await
    }

    pub async fn change_role(
        &self,
        project: &str,
        account: &str,
        to: Role,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Standing<Vec<ListedMember>>, MembersCoreError> {
        let project = ProjectLink::from(project);
        self.authorized(agent, Act::Manage, &project).await?;
        let account = AccountId::from(account);
        if !self
            .catalog
            .leaving(slice::from_ref(&account))
            .await?
            .is_empty()
        {
            return Err(MembersCoreError::Leaving);
        }
        let members = self
            .written(
                &project,
                MembersCommand::ChangeRole { account, to },
                expected,
                agent,
            )
            .await?;

        self.listed(&project, members, agent).await
    }

    pub async fn remove(
        &self,
        project: &str,
        account: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Standing<Vec<Member>>, MembersCoreError> {
        let project = ProjectLink::from(project);
        self.authorized(agent, Act::Manage, &project).await?;

        self.written(
            &project,
            MembersCommand::Remove {
                account: AccountId::from(account),
            },
            expected,
            agent,
        )
        .await
    }

    pub async fn leave(
        &self,
        project: &str,
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<(), MembersCoreError> {
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
            expected,
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
                    None,
                    &Agent::System,
                )
                .await
            {
                Err(MembersCoreError::Events(ServiceError::Refused(
                    MembersError::AlreadyMember | MembersError::Disbanded,
                ))) => {}
                joined => {
                    joined?;
                }
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

    /// Remembers until when an account is leaving, none where it is kept after all, as accounts tell it by the account's version.
    pub async fn remember_leaving(
        &self,
        within: Option<&mut Transaction>,
        account: &AccountId,
        version: Version,
        until: Option<OffsetDateTime>,
    ) -> Result<(), MembersCoreError> {
        transacting::joining!(&*self.transactions, within, |transaction| {
            self.catalog
                .remember_leaving(transaction, account, version, until, self.clock.now())
                .await?;

            Ok(())
        })
    }

    /// Forgets the accounts no longer leaving since before then, once no word about their leaving may be retried.
    pub async fn forget_tombstones_laid_before(
        &self,
        before: OffsetDateTime,
        at_most: u64,
    ) -> Result<u64, MembersCoreError> {
        transacting::retrying!(&*self.transactions, |transaction| {
            let forgotten = self
                .catalog
                .forget_leaving_settled_before(transaction, before, at_most)
                .await?;

            Ok::<_, MembersCoreError>(forgotten)
        })
    }

    /// Lets a deleted account go from every project it is in, as of its version, and forgets who it was; where it was the last owner, the longest-standing other member is made owner, and a project it was alone in is abandoned.
    pub async fn let_go_of(
        &self,
        within: Option<&mut Transaction>,
        account: &AccountId,
        version: Version,
    ) -> Result<(), MembersCoreError> {
        let memberships = self.catalog.memberships_of(account).await?;

        transacting::joining!(&*self.transactions, within, |transaction| {
            for membership in &memberships {
                match self
                    .service
                    .carry_out(
                        transaction,
                        &membership.project,
                        MembersCommand::LetGo {
                            account: account.clone(),
                        },
                        None,
                        &Agent::System,
                    )
                    .await
                {
                    Err(MembersCoreError::Events(ServiceError::Refused(
                        MembersError::Disbanded,
                    ))) => {}
                    let_go => {
                        let_go?;
                    }
                }
            }
            self.catalog.forget_person(transaction, account).await?;
            self.catalog
                .remember_leaving(transaction, account, version, None, self.clock.now())
                .await?;

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
        expected: Option<Version>,
        agent: &Agent,
    ) -> Result<Standing<Vec<Member>>, MembersCoreError> {
        transacting::retrying!(
            &*self.transactions,
            eventsourcing::tries_for(expected),
            |transaction| {
                self.service
                    .carry_out(transaction, project, command.clone(), expected, agent)
                    .await
            }
        )?;
        let standing = self.service.latest(project).await?;

        Ok(Standing {
            state: standing.state.members(),
            version: standing.version,
        })
    }

    async fn listed(
        &self,
        project: &ProjectLink,
        members: Standing<Vec<Member>>,
        agent: &Agent,
    ) -> Result<Standing<Vec<ListedMember>>, MembersCoreError> {
        let Standing {
            state: members,
            version,
        } = members;
        let accounts: Vec<AccountId> = members
            .iter()
            .map(|member| member.account.clone())
            .collect();
        let people = self.catalog.people(&accounts).await?;
        let leaving = self.catalog.leaving(&accounts).await?;
        let owning = self
            .authorizer
            .may(agent, Act::Manage, project.as_str())
            .await?;

        let state = members
            .into_iter()
            .map(|member| {
                let person = people
                    .iter()
                    .find(|person| person.account == member.account);

                ListedMember {
                    name: person.map(|person| person.name.clone()),
                    email: person.filter(|_| owning).map(|person| person.email.clone()),
                    leaving_until: leaving
                        .iter()
                        .find(|gone| gone.account == member.account)
                        .map(|gone| gone.until),
                    account: member.account,
                    role: member.role,
                }
            })
            .collect();

        Ok(Standing { state, version })
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
