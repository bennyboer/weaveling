use std::error::Error;

use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use thiserror::Error;
use transacting::Transaction;

use crate::members::{Member, Members, ProjectLink, Role};

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ProjectMembers {
    pub project: ProjectLink,
    pub version: Version,
    pub members: Vec<Member>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Membership {
    pub project: ProjectLink,
    pub role: Role,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub account: AccountId,
    pub name: String,
    pub email: String,
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("the list of members could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait MembershipCatalog: Send + Sync {
    async fn remember(
        &self,
        transaction: &mut Transaction,
        members: &ProjectMembers,
    ) -> Result<(), CatalogError>;

    async fn role_of(
        &self,
        project: &ProjectLink,
        account: &AccountId,
    ) -> Result<Option<Role>, CatalogError>;

    async fn memberships_of(&self, account: &AccountId) -> Result<Vec<Membership>, CatalogError>;

    async fn remember_person(
        &self,
        transaction: &mut Transaction,
        person: &Person,
    ) -> Result<(), CatalogError>;

    async fn people(&self, accounts: &[AccountId]) -> Result<Vec<Person>, CatalogError>;

    async fn remember_trash(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
        version: Version,
        in_the_trash: bool,
    ) -> Result<(), CatalogError>;

    async fn forget_trash(
        &self,
        transaction: &mut Transaction,
        project: &ProjectLink,
    ) -> Result<(), CatalogError>;

    async fn is_in_the_trash(&self, project: &ProjectLink) -> Result<bool, CatalogError>;
}

impl ProjectMembers {
    pub fn of(project: ProjectLink, version: Version, members: &Members) -> Self {
        Self {
            project,
            version,
            members: members.members(),
        }
    }
}

#[cfg(test)]
mod tests {
    use eventsourcing::{Agent, Aggregate, AggregateContext, AggregateId, EventMetadata};
    use time::OffsetDateTime;

    use super::*;
    use crate::members::{KIND, MembersEvent};

    fn stamped(version: u64) -> EventMetadata {
        EventMetadata {
            aggregate: AggregateId::from("project_1"),
            kind: KIND,
            version: Version::of(version),
            agent: Agent::System,
            occurred_at: OffsetDateTime::UNIX_EPOCH,
            is_snapshot: false,
            context: AggregateContext::default(),
        }
    }

    #[test]
    fn a_project_s_members_are_listed_as_of_their_version() {
        // given: a project's members, an owner and a reader who joined
        let ada = AccountId::from("account_ada");
        let grace = AccountId::from("account_grace");
        let mut members =
            Members::from_first(&MembersEvent::Founded { owner: ada.clone() }, &stamped(1))
                .expect("founding gives birth");
        members.apply(
            &MembersEvent::Joined {
                account: grace.clone(),
                role: Role::Reader,
            },
            &stamped(2),
        );

        // expect: they are listed with their roles as of that version
        assert_eq!(
            ProjectMembers::of(ProjectLink::from("project_1"), Version::of(2), &members),
            ProjectMembers {
                project: ProjectLink::from("project_1"),
                version: Version::of(2),
                members: vec![
                    Member {
                        account: ada,
                        role: Role::Owner
                    },
                    Member {
                        account: grace,
                        role: Role::Reader
                    },
                ],
            }
        );
    }
}
