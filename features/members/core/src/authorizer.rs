use std::collections::BTreeSet;
use std::sync::Arc;

use async_trait::async_trait;
use authorizing::{AccessChanges, AccessWatch, Act, Authorizer, AuthorizerError, Readable};
use eventsourcing::Agent;

use crate::catalog::{CatalogError, MembershipCatalog};
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

fn unreachable(failure: CatalogError) -> AuthorizerError {
    AuthorizerError::Unreachable(Box::new(failure))
}

#[async_trait]
impl Authorizer for MembersAuthorizer {
    async fn may(&self, agent: &Agent, act: Act, project: &str) -> Result<bool, AuthorizerError> {
        match agent {
            Agent::System => Ok(true),
            Agent::Local => Ok(false),
            Agent::Account(account) => {
                let project = ProjectLink::from(project);
                let permitted = self
                    .catalog
                    .role_of(&project, account)
                    .await
                    .map_err(unreachable)?
                    .is_some_and(|role| role.permits(act));
                if !permitted || act == Act::Restore {
                    return Ok(permitted);
                }

                Ok(!self
                    .catalog
                    .is_in_the_trash(&project)
                    .await
                    .map_err(unreachable)?)
            }
        }
    }

    async fn readable_by(&self, agent: &Agent) -> Result<Readable, AuthorizerError> {
        match agent {
            Agent::System => Ok(Readable::Everything),
            Agent::Local => Ok(Readable::Only(BTreeSet::new())),
            Agent::Account(account) => {
                let mut readable = BTreeSet::new();
                for membership in self
                    .catalog
                    .memberships_of(account)
                    .await
                    .map_err(unreachable)?
                {
                    if !self
                        .catalog
                        .is_in_the_trash(&membership.project)
                        .await
                        .map_err(unreachable)?
                    {
                        readable.insert(membership.project.to_string());
                    }
                }

                Ok(Readable::Only(readable))
            }
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

    use eventsourcing::{AccountId, Version};

    use super::*;
    use transacting::Transaction;

    use crate::catalog::{Membership, Person, ProjectMembers};

    struct Kept {
        roles: Mutex<Vec<(String, AccountId, Role)>>,
        trashed: Mutex<Vec<String>>,
        down: bool,
    }

    #[async_trait]
    impl MembershipCatalog for Kept {
        async fn remember(
            &self,
            _transaction: &mut Transaction,
            _members: &ProjectMembers,
        ) -> Result<(), CatalogError> {
            Ok(())
        }

        async fn remember_person(
            &self,
            _transaction: &mut Transaction,
            _person: &Person,
        ) -> Result<(), CatalogError> {
            Ok(())
        }

        async fn people(&self, _accounts: &[AccountId]) -> Result<Vec<Person>, CatalogError> {
            Ok(Vec::new())
        }

        async fn remember_trash(
            &self,
            _transaction: &mut Transaction,
            _project: &ProjectLink,
            _version: Version,
            _in_the_trash: bool,
        ) -> Result<(), CatalogError> {
            Ok(())
        }

        async fn forget_trash(
            &self,
            _transaction: &mut Transaction,
            _project: &ProjectLink,
        ) -> Result<(), CatalogError> {
            Ok(())
        }

        async fn is_in_the_trash(&self, project: &ProjectLink) -> Result<bool, CatalogError> {
            Ok(self
                .trashed
                .lock()
                .expect("never poisoned")
                .iter()
                .any(|trashed| trashed == project.as_str()))
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

    fn an_authorizer(roles: Vec<(&str, AccountId, Role)>) -> MembersAuthorizer {
        MembersAuthorizer::new(Arc::new(Kept {
            roles: Mutex::new(
                roles
                    .into_iter()
                    .map(|(project, account, role)| (project.to_owned(), account, role))
                    .collect(),
            ),
            trashed: Mutex::new(Vec::new()),
            down: false,
        }))
    }

    fn an_authorizer_with_trashed(
        roles: Vec<(&str, AccountId, Role)>,
        trashed: &[&str],
    ) -> MembersAuthorizer {
        MembersAuthorizer::new(Arc::new(Kept {
            roles: Mutex::new(
                roles
                    .into_iter()
                    .map(|(project, account, role)| (project.to_owned(), account, role))
                    .collect(),
            ),
            trashed: Mutex::new(
                trashed
                    .iter()
                    .map(|project| (*project).to_owned())
                    .collect(),
            ),
            down: false,
        }))
    }

    async fn allowed(authorizer: &MembersAuthorizer, agent: &Agent, project: &str) -> Vec<Act> {
        let mut allowed = Vec::new();
        for act in [Act::Read, Act::Write, Act::Manage, Act::Restore] {
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
            (
                Role::Owner,
                vec![Act::Read, Act::Write, Act::Manage, Act::Restore],
            ),
            (Role::Author, vec![Act::Read, Act::Write]),
            (Role::Reader, vec![Act::Read]),
        ] {
            // given: ada in a project in each role
            let authorizer = an_authorizer(vec![("project_1", ada(), role)]);

            // expect: she may do what the role allows
            assert_eq!(
                allowed(&authorizer, &Agent::Account(ada()), "project_1").await,
                acts,
                "{role:?}"
            );
        }
    }

    #[tokio::test]
    async fn a_project_in_the_trash_is_read_written_and_managed_by_nobody_and_restored_by_its_owners()
     {
        for (role, acts) in [
            (Role::Owner, vec![Act::Restore]),
            (Role::Author, vec![]),
            (Role::Reader, vec![]),
        ] {
            // given: ada in a project in the trash in each role
            let authorizer =
                an_authorizer_with_trashed(vec![("project_1", ada(), role)], &["project_1"]);

            // expect: she may only restore it, and only as owner
            assert_eq!(
                allowed(&authorizer, &Agent::Account(ada()), "project_1").await,
                acts,
                "{role:?}: only an owner may restore what is in the trash"
            );
        }
    }

    #[tokio::test]
    async fn the_application_still_acts_on_a_project_in_the_trash() {
        // given: a project in the trash
        let authorizer = an_authorizer_with_trashed(vec![], &["project_1"]);

        // expect: the application may do everything in it
        assert_eq!(
            allowed(&authorizer, &Agent::System, "project_1").await,
            vec![Act::Read, Act::Write, Act::Manage, Act::Restore],
            "the purge deletes it as the system"
        );
    }

    #[tokio::test]
    async fn a_project_in_the_trash_is_readable_by_nobody() {
        // given: ada owning two projects, one of them in the trash
        let authorizer = an_authorizer_with_trashed(
            vec![
                ("project_1", ada(), Role::Owner),
                ("project_2", ada(), Role::Owner),
            ],
            &["project_1"],
        );

        // expect: she reads only the other
        assert_eq!(
            authorizer
                .readable_by(&Agent::Account(ada()))
                .await
                .expect("asking should succeed"),
            Readable::Only(BTreeSet::from(["project_2".to_owned()]))
        );
    }

    #[tokio::test]
    async fn an_account_may_do_nothing_in_a_project_it_is_not_in() {
        // given: ada in one project and grace in another
        let authorizer = an_authorizer(vec![
            ("project_1", ada(), Role::Owner),
            ("project_2", AccountId::from("account_grace"), Role::Owner),
        ]);

        // expect: ada may do nothing in grace's project
        assert!(
            allowed(&authorizer, &Agent::Account(ada()), "project_2")
                .await
                .is_empty()
        );
        // and: nothing in a project nobody is in
        assert!(
            allowed(&authorizer, &Agent::Account(ada()), "project_3")
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn on_a_server_the_application_may_act_and_nobody_unsigned_may() {
        // given: a project nobody is in
        let authorizer = an_authorizer(vec![]);

        // expect: the application may do everything in it
        assert_eq!(
            allowed(&authorizer, &Agent::System, "project_1").await,
            vec![Act::Read, Act::Write, Act::Manage, Act::Restore]
        );
        // and: nobody unsigned may do anything
        assert!(
            allowed(&authorizer, &Agent::Local, "project_1")
                .await
                .is_empty()
        );
    }

    #[tokio::test]
    async fn a_list_that_cannot_be_asked_is_a_failure_not_a_no() {
        // given: a list of members that is down
        let authorizer = MembersAuthorizer::new(Arc::new(Kept {
            roles: Mutex::new(vec![]),
            trashed: Mutex::new(Vec::new()),
            down: true,
        }));

        // expect: asking what an account may do fails
        assert!(
            authorizer
                .may(&Agent::Account(ada()), Act::Read, "project_1")
                .await
                .is_err()
        );
    }

    #[tokio::test]
    async fn an_account_reads_every_project_it_is_in_whatever_its_role_and_no_other() {
        // given: ada in two projects in different roles and grace in a third
        let authorizer = an_authorizer(vec![
            ("project_1", ada(), Role::Reader),
            ("project_2", ada(), Role::Owner),
            ("project_3", AccountId::from("account_grace"), Role::Owner),
        ]);

        // expect: ada reads her two
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
        // given: a project
        let authorizer = an_authorizer(vec![("project_1", ada(), Role::Owner)]);

        // expect: the application reads everything
        assert_eq!(
            authorizer
                .readable_by(&Agent::System)
                .await
                .expect("nothing to ask"),
            Readable::Everything
        );
        // and: nobody unsigned reads anything
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
        // given: a list of members that is down
        let authorizer = MembersAuthorizer::new(Arc::new(Kept {
            roles: Mutex::new(vec![]),
            trashed: Mutex::new(Vec::new()),
            down: true,
        }));

        // expect: asking what an account may read fails
        assert!(
            authorizer
                .readable_by(&Agent::Account(ada()))
                .await
                .is_err()
        );
    }
}
