use accounts_contract::{AccountEventDTO, DELETED};
use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use inbox::KeptHandler;
use members_core::MembersCore;
use messaging::{Failure, Message};
use transacting::Transaction;

pub struct LetGoOfDeletedAccountListener {
    members: MembersCore,
}

impl LetGoOfDeletedAccountListener {
    pub fn new(members: MembersCore) -> Self {
        Self { members }
    }
}

#[async_trait]
impl KeptHandler for LetGoOfDeletedAccountListener {
    const NAME: &'static str = "let-go-of-deleted-account";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str = "A deleted account was not removed from its projects. It is still listed as a member, maybe even as a project's last owner. Its name and address are still stored.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<AccountEventDTO>(message)?;
        let AccountEventDTO::Deleted = told.event.body else {
            return Err("this tells no account deleted".into());
        };

        Ok(self
            .members
            .let_go_of(
                Some(transaction),
                &AccountId::from(told.aggregate.id),
                Version::of(told.aggregate.version),
            )
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use authorizing::AllowingEverything;
    use clock::FixedClock;
    use eventsourcing::{Agent, AggregateId, EventStore, InMemoryEventStore};
    use inbox::{InMemoryInbox, Inboxed};
    use members_catalog::InMemoryMembershipCatalog;
    use members_core::{KIND, MembersEvent, MembershipCatalog, Person, ProjectLink, Role};
    use messaging::{Listener, RoutingKey};
    use serde_json::json;
    use time::OffsetDateTime;
    use transacting::InMemoryTransactions;

    use super::*;

    struct Bench {
        store: Arc<InMemoryEventStore<MembersEvent>>,
        catalog: Arc<InMemoryMembershipCatalog>,
        members: MembersCore,
        listener: Inboxed<LetGoOfDeletedAccountListener>,
    }

    fn bench() -> Bench {
        let store = Arc::new(InMemoryEventStore::new());
        let catalog = Arc::new(InMemoryMembershipCatalog::new());
        let members = MembersCore::new(
            store.clone(),
            Arc::new(InMemoryTransactions::new()),
            catalog.clone(),
            Arc::new(AllowingEverything),
            Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        );

        Bench {
            listener: Inboxed::new(
                LetGoOfDeletedAccountListener::new(members.clone()),
                Arc::new(InMemoryTransactions::new()),
                Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(
                    OffsetDateTime::UNIX_EPOCH,
                )))),
            ),
            store,
            catalog,
            members,
        }
    }

    fn ada() -> AccountId {
        AccountId::from("account_ada")
    }

    fn deleted(event: serde_json::Value) -> Message {
        eventpublishing::published(
            DELETED,
            "account",
            "account_ada",
            9,
            event,
            &Agent::System,
            OffsetDateTime::UNIX_EPOCH,
        )
    }

    async fn in_project(bench: &Bench, project: &str, joining: &[(&str, Role)]) {
        bench
            .members
            .found(None, project, "account_ada")
            .await
            .expect("founding should succeed");
        for (account, role) in joining {
            bench
                .members
                .join(None, project, account, *role)
                .await
                .expect("joining should succeed");
        }
        bench
            .members
            .catalogue(None, &ProjectLink::from(project))
            .await
            .expect("cataloguing should succeed");
    }

    async fn told_since_founding(bench: &Bench, project: &str, joined: u64) -> Vec<MembersEvent> {
        bench
            .store
            .read_from(
                &AggregateId::from(&ProjectLink::from(project)),
                KIND,
                Version::of(joined + 2),
            )
            .await
            .expect("reading should succeed")
            .into_iter()
            .map(|recorded| recorded.event)
            .collect()
    }

    #[tokio::test]
    async fn a_deleted_account_is_let_go_of_every_project_and_forgotten() {
        // given: the listener
        let bench = bench();
        // expect: it hears an account deleted
        assert!(
            bench
                .listener
                .hears(&RoutingKey::parse(DELETED).expect("a declared key"))
        );
        // given: ada the only owner of one project with grace in it, and alone in another
        in_project(&bench, "project_shared", &[("account_grace", Role::Reader)]).await;
        in_project(&bench, "project_alone", &[]).await;
        // and: she is known, and leaving
        bench
            .members
            .remember_person(
                None,
                &Person {
                    account: ada(),
                    name: "Ada Lovelace".to_owned(),
                    email: "ada@example.com".to_owned(),
                },
            )
            .await
            .expect("remembering should succeed");
        bench
            .members
            .remember_leaving(
                None,
                &ada(),
                Version::of(7),
                Some(OffsetDateTime::UNIX_EPOCH),
            )
            .await
            .expect("remembering should succeed");

        // when: her account is heard deleted, twice
        for _ in 0..2 {
            bench
                .listener
                .handle(&deleted(json!({ "version": 9, "name": "DELETED" })))
                .await
                .expect("messages come at least once");
        }

        // then: grace owns the shared project and ada is gone from it
        assert_eq!(
            told_since_founding(&bench, "project_shared", 1).await,
            vec![
                MembersEvent::RoleChanged {
                    account: AccountId::from("account_grace"),
                    to: Role::Owner,
                },
                MembersEvent::Removed { account: ada() },
            ]
        );
        // and: the project she was alone in is abandoned
        assert_eq!(
            told_since_founding(&bench, "project_alone", 0).await,
            vec![
                MembersEvent::Removed { account: ada() },
                MembersEvent::Abandoned,
            ]
        );
        // and: she is known no more, nor leaving
        assert!(
            bench
                .catalog
                .people(&[ada()])
                .await
                .expect("asking should succeed")
                .is_empty()
        );
        assert!(
            bench
                .catalog
                .leaving(&[ada()])
                .await
                .expect("asking should succeed")
                .is_empty()
        );
    }

    #[tokio::test]
    async fn what_tells_no_deletion_is_refused() {
        // given: the listener
        let bench = bench();

        // expect: hearing what is no deletion fails
        assert!(
            bench
                .listener
                .handle(&deleted(json!({ "version": 9, "name": "EMAIL_VERIFIED" })))
                .await
                .is_err(),
            "a message the listener cannot act on belongs in dead letters"
        );
    }
}
