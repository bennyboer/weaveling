use async_trait::async_trait;
use inbox::KeptHandler;
use members_core::MembersCore;
use messaging::{Failure, Message};
use projects_contract::{DELETED, ProjectEventDTO};
use transacting::Transaction;

pub struct DisbandMembersOfDeletedProjectListener {
    members: MembersCore,
}

impl DisbandMembersOfDeletedProjectListener {
    pub fn new(members: MembersCore) -> Self {
        Self { members }
    }
}

#[async_trait]
impl KeptHandler for DisbandMembersOfDeletedProjectListener {
    const NAME: &'static str = "disband-members-of-deleted-project";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str =
        "The members of a deleted project were not let go, so its owners may still invite into it.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let deleted = eventpublishing::published_in::<ProjectEventDTO>(message)?;

        Ok(self
            .members
            .disband(Some(transaction), &deleted.aggregate.id)
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use authorizing::AllowingEverything;
    use clock::FixedClock;
    use eventsourcing::{AccountId, Agent, AggregateId, EventStore, InMemoryEventStore, Version};
    use members_catalog::InMemoryMembershipCatalog;
    use members_core::{KIND, MembersEvent, ProjectLink, Role};
    use messaging::{Listener, RoutingKey};
    use serde_json::json;
    use time::OffsetDateTime;
    use transacting::InMemoryTransactions;

    use inbox::{InMemoryInbox, Inboxed};

    use super::*;

    struct Bench {
        store: Arc<InMemoryEventStore<MembersEvent>>,
        members: MembersCore,
        listener: Inboxed<DisbandMembersOfDeletedProjectListener>,
    }

    fn bench() -> Bench {
        bench_hearing_in(InMemoryTransactions::new())
    }

    fn bench_hearing_in(hearings: InMemoryTransactions) -> Bench {
        let store = Arc::new(InMemoryEventStore::new());
        let members = MembersCore::new(
            store.clone(),
            Arc::new(InMemoryTransactions::new()),
            Arc::new(InMemoryMembershipCatalog::new()),
            Arc::new(AllowingEverything),
            Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        );

        Bench {
            listener: Inboxed::new(
                DisbandMembersOfDeletedProjectListener::new(members.clone()),
                Arc::new(hearings),
                Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(
                    OffsetDateTime::UNIX_EPOCH,
                )))),
            ),
            members,
            store,
        }
    }

    fn a_deletion() -> Message {
        eventpublishing::published(
            DELETED,
            "project",
            "project_1",
            4,
            json!({ "version": 0, "name": "DELETED" }),
            &Agent::Account(AccountId::from("account_ada")),
            OffsetDateTime::UNIX_EPOCH,
        )
    }

    async fn events_of(bench: &Bench) -> Vec<(MembersEvent, Agent)> {
        bench
            .store
            .read_from(
                &AggregateId::from(&ProjectLink::from("project_1")),
                KIND,
                Version::of(1),
            )
            .await
            .expect("reading should succeed")
            .into_iter()
            .map(|recorded| (recorded.event, recorded.metadata.agent))
            .collect()
    }

    #[tokio::test]
    async fn a_deleted_project_s_members_are_disbanded_by_the_system_once_however_often_it_is_told()
    {
        // given: the listener
        let bench = bench();
        // expect: it hears a project's deletion
        assert!(
            bench
                .listener
                .hears(&RoutingKey::parse(DELETED).expect("a declared key"))
        );

        // given: a project's members, an owner and a reader
        bench
            .members
            .found(None, "project_1", "account_ada")
            .await
            .expect("founding should succeed");
        bench
            .members
            .join(None, "project_1", "account_bo", Role::Reader)
            .await
            .expect("joining should succeed");

        // when: its deletion is heard twice
        for _ in 0..2 {
            bench
                .listener
                .handle(&a_deletion())
                .await
                .expect("messages come at least once");
        }

        // then: the members are disbanded once, by the system
        let told = events_of(&bench).await;
        assert_eq!(told.len(), 3);
        assert_eq!(told[2], (MembersEvent::Disbanded, Agent::System));
    }

    #[tokio::test]
    async fn a_deletion_heard_before_the_founding_leaves_nothing_to_found_and_nobody_to_join() {
        // given: the listener
        let bench = bench();

        // when: a project's deletion is heard
        bench
            .listener
            .handle(&a_deletion())
            .await
            .expect("members never founded are disbanded all the same");
        // and: then its founding and a joining
        bench
            .members
            .found(None, "project_1", "account_ada")
            .await
            .expect("a founding heard late is no failure");
        bench
            .members
            .join(None, "project_1", "account_bo", Role::Reader)
            .await
            .expect("an invitation taken up into a deleted project is no failure either");

        // then: its members were only disbanded
        assert_eq!(
            events_of(&bench).await,
            vec![(MembersEvent::Disbanded, Agent::System)]
        );
    }

    #[tokio::test]
    async fn a_disbanding_whose_hearing_does_not_commit_is_undone_with_it() {
        // given: a listener whose hearings do not commit
        let bench = bench_hearing_in(InMemoryTransactions::refusing_commits());
        // and: a project's members founded
        bench
            .members
            .found(None, "project_1", "account_ada")
            .await
            .expect("founding should succeed");

        // when: its deletion is heard
        bench
            .listener
            .handle(&a_deletion())
            .await
            .expect_err("a hearing that does not commit is not handled");

        // then: its members are only founded
        assert_eq!(
            events_of(&bench).await.len(),
            1,
            "the disbanding is written in the hearing's transaction"
        );
    }
}
