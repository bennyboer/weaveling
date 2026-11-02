use async_trait::async_trait;
use eventsourcing::Version;
use inbox::KeptHandler;
use members_core::{MembersCore, ProjectLink};
use messaging::{Failure, Message};
use projects_contract::{DELETED, ProjectEventDTO, RESTORED, TRASHED};
use transacting::Transaction;

pub struct RememberTrashedProjectsListener {
    members: MembersCore,
}

impl RememberTrashedProjectsListener {
    pub fn new(members: MembersCore) -> Self {
        Self { members }
    }
}

#[async_trait]
impl KeptHandler for RememberTrashedProjectsListener {
    const NAME: &'static str = "members-remember-trashed-projects";
    const LISTENS_TO: &'static [&'static str] = &[TRASHED, RESTORED, DELETED];
    const WHEN_REFUSED: &'static str = "A project put in the trash or restored did not reach its members, so who may read and write it is not as it should be.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<ProjectEventDTO>(message)?;
        let project = ProjectLink::from(told.aggregate.id.as_str());
        let in_the_trash = match told.event.body {
            ProjectEventDTO::Trashed { .. } => true,
            ProjectEventDTO::Restored => false,
            ProjectEventDTO::Deleted => {
                return Ok(self
                    .members
                    .forget_trash(Some(transaction), &project)
                    .await?);
            }
            _ => return Err("this tells neither a trashing, a restoring nor a deletion".into()),
        };

        Ok(self
            .members
            .remember_trash(
                Some(transaction),
                &project,
                Version::of(told.aggregate.version),
                in_the_trash,
            )
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::{Arc, Mutex};

    use async_trait::async_trait;
    use authorizing::{AccessWatch, Act, Authorizer, AuthorizerError, Readable};
    use clock::FixedClock;
    use eventsourcing::{AccountId, Agent, InMemoryEventStore};
    use inbox::{InMemoryInbox, Inboxed};
    use members_catalog::InMemoryMembershipCatalog;
    use members_core::{Member, MembersAuthorizer, MembershipCatalog, ProjectMembers, Role};
    use messaging::{Listener, RoutingKey};
    use serde_json::json;
    use time::OffsetDateTime;
    use transacting::{InMemoryTransactions, Transactions};

    use super::*;

    struct Telling {
        inner: MembersAuthorizer,
        told: Mutex<Vec<String>>,
    }

    #[async_trait]
    impl Authorizer for Telling {
        async fn may(
            &self,
            agent: &Agent,
            act: Act,
            project: &str,
        ) -> Result<bool, AuthorizerError> {
            self.inner.may(agent, act, project).await
        }

        async fn readable_by(&self, agent: &Agent) -> Result<Readable, AuthorizerError> {
            self.inner.readable_by(agent).await
        }

        fn access_changed(&self, project: &str) {
            self.told
                .lock()
                .expect("nothing panics holding it")
                .push(project.to_owned());
        }

        fn watch_access(&self, project: &str) -> AccessWatch {
            self.inner.watch_access(project)
        }
    }

    struct Bench {
        catalog: Arc<InMemoryMembershipCatalog>,
        authorizer: Arc<Telling>,
        members: MembersCore,
        listener: Inboxed<RememberTrashedProjectsListener>,
    }

    fn bench_hearing_in(hearings: InMemoryTransactions) -> Bench {
        let catalog = Arc::new(InMemoryMembershipCatalog::new());
        let authorizer = Arc::new(Telling {
            inner: MembersAuthorizer::new(catalog.clone()),
            told: Mutex::new(Vec::new()),
        });
        let members = MembersCore::new(
            Arc::new(InMemoryEventStore::new()),
            Arc::new(InMemoryTransactions::new()),
            catalog.clone(),
            authorizer.clone(),
            Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        );

        Bench {
            listener: Inboxed::new(
                RememberTrashedProjectsListener::new(members.clone()),
                Arc::new(hearings),
                Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(
                    OffsetDateTime::UNIX_EPOCH,
                )))),
            ),
            catalog,
            authorizer,
            members,
        }
    }

    fn bench() -> Bench {
        bench_hearing_in(InMemoryTransactions::new())
    }

    fn told(routing: &str, name: &str, version: u64) -> Message {
        Message::opening(
            RoutingKey::parse(routing).expect("a declared key"),
            json!({
                "event": { "version": 0, "name": name, "until": "1970-01-31T00:00:00Z" },
                "aggregate": { "id": "project_1", "kind": "project", "version": version },
                "agent": { "kind": "account", "id": "account_ada" },
                "occurred_at": "1970-01-01T00:00:00Z",
            }),
            OffsetDateTime::UNIX_EPOCH,
        )
    }

    fn trashed(version: u64) -> Message {
        told(TRASHED, "TRASHED", version)
    }

    fn restored(version: u64) -> Message {
        told(RESTORED, "RESTORED", version)
    }

    fn deleted(version: u64) -> Message {
        told(DELETED, "DELETED", version)
    }

    impl Bench {
        async fn in_the_trash(&self) -> bool {
            self.catalog
                .is_in_the_trash(&ProjectLink::from("project_1"))
                .await
                .expect("asking should succeed")
        }

        fn told(&self) -> Vec<String> {
            self.authorizer
                .told
                .lock()
                .expect("nothing panics holding it")
                .clone()
        }
    }

    #[tokio::test]
    async fn a_project_trashed_is_kept_in_the_trash_until_it_is_restored() {
        let bench = bench();
        assert!(
            bench
                .listener
                .hears(&RoutingKey::parse(TRASHED).expect("a declared key"))
        );
        assert!(
            bench
                .listener
                .hears(&RoutingKey::parse(RESTORED).expect("a declared key"))
        );

        bench
            .listener
            .handle(&trashed(5))
            .await
            .expect("hearing should succeed");
        let then = bench.in_the_trash().await;
        bench
            .listener
            .handle(&restored(6))
            .await
            .expect("hearing should succeed");

        assert!(then);
        assert!(!bench.in_the_trash().await);
        assert_eq!(
            bench.told(),
            ["project_1", "project_1"],
            "access changed each time, so open sockets ask again"
        );
    }

    #[tokio::test]
    async fn a_project_deleted_once_its_time_in_the_trash_is_over_leaves_nothing_behind() {
        let bench = bench();
        assert!(
            bench
                .listener
                .hears(&RoutingKey::parse(DELETED).expect("a declared key"))
        );

        for message in [trashed(5), deleted(6)] {
            bench
                .listener
                .handle(&message)
                .await
                .expect("hearing should succeed");
        }

        assert!(
            !bench.in_the_trash().await,
            "a deleted project's trash is forgotten, so the table holds only what is in it"
        );
    }

    #[tokio::test]
    async fn an_older_trashing_heard_after_its_restoring_leaves_the_project_out_of_the_trash() {
        let bench = bench();

        for message in [restored(6), trashed(5)] {
            bench
                .listener
                .handle(&message)
                .await
                .expect("hearing should succeed");
        }

        assert!(!bench.in_the_trash().await);
    }

    #[tokio::test]
    async fn a_hearing_that_does_not_commit_leaves_no_trace_and_tells_nobody() {
        let bench = bench_hearing_in(InMemoryTransactions::refusing_commits());

        bench
            .listener
            .handle(&trashed(5))
            .await
            .expect_err("a hearing that does not commit is not handled");

        assert!(
            !bench.in_the_trash().await,
            "written in the hearing's transaction"
        );
        assert!(
            bench.told().is_empty(),
            "access is told only once committed"
        );
    }

    #[tokio::test]
    async fn an_author_loses_a_project_to_the_trash_and_its_owner_may_only_restore_it() {
        let bench = bench();
        let ada = Agent::Account(AccountId::from("account_ada"));
        let mut transaction = InMemoryTransactions::new()
            .begin()
            .await
            .expect("beginning should succeed");
        bench
            .catalog
            .remember(
                &mut transaction,
                &ProjectMembers {
                    project: ProjectLink::from("project_1"),
                    version: Version::of(1),
                    members: vec![Member {
                        account: AccountId::from("account_ada"),
                        role: Role::Owner,
                    }],
                },
            )
            .await
            .expect("remembering should succeed");
        transaction
            .finish()
            .await
            .expect("finishing should succeed");

        bench
            .listener
            .handle(&trashed(5))
            .await
            .expect("hearing should succeed");

        let authorizer = &bench.authorizer.inner;
        assert!(
            !authorizer
                .may(&ada, Act::Read, "project_1")
                .await
                .expect("asking")
        );
        assert!(
            !authorizer
                .may(&ada, Act::Manage, "project_1")
                .await
                .expect("asking"),
            "an owner manages nothing in a project in the trash, its members and invitations as they were"
        );
        assert!(
            bench
                .members
                .change_role("project_1", "account_grace", Role::Owner, &ada)
                .await
                .is_err(),
            "no role changes in a project in the trash"
        );
        assert!(
            bench
                .members
                .remove("project_1", "account_ada", &ada)
                .await
                .is_err(),
            "nor is anyone removed from it"
        );
        assert!(
            authorizer
                .may(&ada, Act::Restore, "project_1")
                .await
                .expect("asking")
        );
    }
}
