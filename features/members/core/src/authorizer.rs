use std::sync::Arc;

use async_trait::async_trait;
use authorizing::{Act, Authorizer, AuthorizerError};
use eventsourcing::Agent;

use crate::catalog::MembershipCatalog;
use crate::members::{ProjectLink, Role};

pub struct MembersAuthorizer {
    catalog: Arc<dyn MembershipCatalog>,
}

impl MembersAuthorizer {
    pub fn new(catalog: Arc<dyn MembershipCatalog>) -> Self {
        Self { catalog }
    }
}

impl Role {
    pub fn permits(self, act: Act) -> bool {
        match self {
            Role::Owner => true,
            Role::Author => matches!(act, Act::Read | Act::Write),
            Role::Reader => act == Act::Read,
        }
    }
}

#[async_trait]
impl Authorizer for MembersAuthorizer {
    async fn may(&self, agent: &Agent, act: Act, project: &str) -> Result<bool, AuthorizerError> {
        match agent {
            Agent::System => Ok(true),
            Agent::Local => Ok(false),
            Agent::Account(account) => Ok(self
                .catalog
                .role_of(&ProjectLink::from(project), account)
                .await
                .map_err(|failure| AuthorizerError::Unreachable(Box::new(failure)))?
                .is_some_and(|role| role.permits(act))),
        }
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use eventsourcing::AccountId;

    use super::*;
    use crate::catalog::{CatalogError, Membership, ProjectMembers};

    struct Kept {
        roles: Mutex<Vec<(String, AccountId, Role)>>,
        down: bool,
    }

    #[async_trait]
    impl MembershipCatalog for Kept {
        async fn remember(&self, _members: &ProjectMembers) -> Result<(), CatalogError> {
            Ok(())
        }

        async fn role_of(
            &self,
            project: &ProjectLink,
            account: &AccountId,
        ) -> Result<Option<Role>, CatalogError> {
            if self.down {
                return Err(CatalogError::Backend("the list is down".into()));
            }

            Ok(self
                .roles
                .lock()
                .expect("never poisoned")
                .iter()
                .find(|(kept, member, _)| kept == project.as_str() && member == account)
                .map(|(_, _, role)| *role))
        }

        async fn memberships_of(
            &self,
            _account: &AccountId,
        ) -> Result<Vec<Membership>, CatalogError> {
            Ok(vec![])
        }
    }

    fn ada() -> AccountId {
        AccountId::from("account_ada")
    }

    fn authorizer(roles: Vec<(&str, AccountId, Role)>) -> MembersAuthorizer {
        MembersAuthorizer::new(Arc::new(Kept {
            roles: Mutex::new(
                roles
                    .into_iter()
                    .map(|(project, account, role)| (project.to_owned(), account, role))
                    .collect(),
            ),
            down: false,
        }))
    }

    async fn allowed(authorizer: &MembersAuthorizer, agent: &Agent, project: &str) -> Vec<Act> {
        let mut allowed = Vec::new();
        for act in [Act::Read, Act::Write, Act::Manage] {
            if authorizer
                .may(agent, act, project)
                .await
                .expect("asking should succeed")
            {
                allowed.push(act);
            }
        }
        allowed
    }

    #[tokio::test]
    async fn an_owner_does_everything_an_author_reads_and_writes_a_reader_reads() {
        for (role, acts) in [
            (Role::Owner, vec![Act::Read, Act::Write, Act::Manage]),
            (Role::Author, vec![Act::Read, Act::Write]),
            (Role::Reader, vec![Act::Read]),
        ] {
            let authorizer = authorizer(vec![("project_1", ada(), role)]);

            assert_eq!(
                allowed(&authorizer, &Agent::Account(ada()), "project_1").await,
                acts,
                "{role:?}"
            );
        }
    }

    #[tokio::test]
    async fn an_account_may_do_nothing_in_a_project_it_is_not_in() {
        let authorizer = authorizer(vec![
            ("project_1", ada(), Role::Owner),
            ("project_2", AccountId::from("account_grace"), Role::Owner),
        ]);

        assert!(
            allowed(&authorizer, &Agent::Account(ada()), "project_2")
                .await
                .is_empty()
        );
        assert!(
            allowed(&authorizer, &Agent::Account(ada()), "project_3")
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn on_a_server_the_application_may_act_and_nobody_unsigned_may() {
        let authorizer = authorizer(vec![]);

        assert_eq!(
            allowed(&authorizer, &Agent::System, "project_1").await,
            vec![Act::Read, Act::Write, Act::Manage]
        );
        assert!(
            allowed(&authorizer, &Agent::Local, "project_1")
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_list_that_cannot_be_asked_is_a_failure_not_a_no() {
        let authorizer = MembersAuthorizer::new(Arc::new(Kept {
            roles: Mutex::new(vec![]),
            down: true,
        }));

        assert!(
            authorizer
                .may(&Agent::Account(ada()), Act::Read, "project_1")
                .await
                .is_err()
        );
    }
}
