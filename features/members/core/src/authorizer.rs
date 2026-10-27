use std::collections::BTreeSet;
use std::sync::Arc;

use async_trait::async_trait;
use authorizing::{AccessChanges, AccessWatch, Act, Authorizer, AuthorizerError, Readable};
use eventsourcing::Agent;

use crate::catalog::MembershipCatalog;
use crate::members::{ProjectLink, Role};

pub struct MembersAuthorizer {
    catalog: Arc<dyn MembershipCatalog>,
    changes: AccessChanges,
}

impl MembersAuthorizer {
    pub fn new(catalog: Arc<dyn MembershipCatalog>) -> Self {
        Self {
            catalog,
            changes: AccessChanges::new(),
        }
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

    async fn readable_by(&self, agent: &Agent) -> Result<Readable, AuthorizerError> {
        match agent {
            Agent::System => Ok(Readable::Everything),
            Agent::Local => Ok(Readable::Only(BTreeSet::new())),
            Agent::Account(account) => Ok(Readable::Only(
                self.catalog
                    .memberships_of(account)
                    .await
                    .map_err(|failure| AuthorizerError::Unreachable(Box::new(failure)))?
                    .into_iter()
                    .map(|membership| membership.project.to_string())
                    .collect(),
            )),
        }
    }

    fn access_changed(&self, project: &str) {
        self.changes.tell(project);
    }

    fn watch_access(&self, project: &str) -> AccessWatch {
        self.changes.watch(project)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Mutex;

    use eventsourcing::AccountId;

    use super::*;
    use crate::catalog::{CatalogError, Membership, Person, ProjectMembers};

    struct Kept {
        roles: Mutex<Vec<(String, AccountId, Role)>>,
        down: bool,
    }

    #[async_trait]
    impl MembershipCatalog for Kept {
        async fn remember(&self, _members: &ProjectMembers) -> Result<(), CatalogError> {
            Ok(())
        }

        async fn remember_person(&self, _person: &Person) -> Result<(), CatalogError> {
            Ok(())
        }

        async fn people(&self, _accounts: &[AccountId]) -> Result<Vec<Person>, CatalogError> {
            Ok(Vec::new())
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
            account: &AccountId,
        ) -> Result<Vec<Membership>, CatalogError> {
            if self.down {
                return Err(CatalogError::Backend("the list is down".into()));
            }

            Ok(self
                .roles
                .lock()
                .expect("never poisoned")
                .iter()
                .filter(|(_, member, _)| member == account)
                .map(|(project, _, role)| Membership {
                    project: ProjectLink::from(project.as_str()),
                    role: *role,
                })
                .collect())
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

    #[tokio::test]
    async fn an_account_reads_every_project_it_is_in_whatever_its_role_and_no_other() {
        let authorizer = authorizer(vec![
            ("project_1", ada(), Role::Reader),
            ("project_2", ada(), Role::Owner),
            ("project_3", AccountId::from("account_grace"), Role::Owner),
        ]);

        assert_eq!(
            authorizer
                .readable_by(&Agent::Account(ada()))
                .await
                .expect("asking should succeed"),
            Readable::Only(BTreeSet::from([
                "project_1".to_owned(),
                "project_2".to_owned()
            ]))
        );
    }

    #[tokio::test]
    async fn on_a_server_the_application_reads_everything_and_nobody_unsigned_anything() {
        let authorizer = authorizer(vec![("project_1", ada(), Role::Owner)]);

        assert_eq!(
            authorizer
                .readable_by(&Agent::System)
                .await
                .expect("nothing to ask"),
            Readable::Everything
        );
        assert_eq!(
            authorizer
                .readable_by(&Agent::Local)
                .await
                .expect("nothing to ask"),
            Readable::Only(BTreeSet::new())
        );
    }

    #[tokio::test]
    async fn a_list_that_cannot_be_asked_is_a_failure_not_nothing_to_read() {
        let authorizer = MembersAuthorizer::new(Arc::new(Kept {
            roles: Mutex::new(vec![]),
            down: true,
        }));

        assert!(
            authorizer
                .readable_by(&Agent::Account(ada()))
                .await
                .is_err()
        );
    }
}
