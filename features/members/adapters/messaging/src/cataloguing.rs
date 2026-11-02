use async_trait::async_trait;
use inbox::{KeptHandler, Refused};
use members_contract::EVERY_MEMBERS;
use members_core::{MembersCore, ProjectLink};
use messaging::Message;
use transacting::Transaction;

pub struct CatalogueMembersListener {
    members: MembersCore,
}

impl CatalogueMembersListener {
    pub fn new(members: MembersCore) -> Self {
        Self { members }
    }
}

#[async_trait]
impl KeptHandler for CatalogueMembersListener {
    const NAME: &'static str = "catalogue-members";
    const LISTENS_TO: &'static [&'static str] = &[EVERY_MEMBERS];
    const WHEN_REFUSED: &'static str =
        "A change to a project's members did not reach the list of who may work where.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Refused> {
        self.members
            .catalogue(
                Some(transaction),
                &eventpublishing::aggregate_in::<ProjectLink>(message)?,
            )
            .await
            .map_err(Refused::of)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use authorizing::AllowingEverything;
    use clock::{Clock, FixedClock};
    use eventsourcing::{
        AccountId, Agent, AggregateId, EventSourcingService, EventStore, Recorded,
    };
    use members_catalog::InMemoryMembershipCatalog;
    use members_core::{Members, MembersCommand, MembersEvent, MembershipCatalog, Role};
    use messaging::{Listener, RoutingKey};
    use serde_json::json;
    use time::OffsetDateTime;
    use transacting::{InMemoryTransactions, Transactions};

    use inbox::{InMemoryInbox, Inboxed};

    use super::*;

    struct Bench {
        events: EventSourcingService<Members>,
        catalog: Arc<InMemoryMembershipCatalog>,
        listener: Inboxed<CatalogueMembersListener>,
    }

    async fn begun() -> Transaction {
        InMemoryTransactions::new()
            .begin()
            .await
            .expect("beginning should succeed")
    }

    fn bench() -> Bench {
        let store: Arc<dyn EventStore<MembersEvent>> =
            Arc::new(eventsourcing::InMemoryEventStore::new());
        let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH));
        let catalog = Arc::new(InMemoryMembershipCatalog::new());
        let transactions: Arc<dyn Transactions> = Arc::new(InMemoryTransactions::new());

        Bench {
            events: EventSourcingService::new(store.clone(), clock.clone()),
            listener: Inboxed::new(
                CatalogueMembersListener::new(MembersCore::new(
                    store,
                    transactions.clone(),
                    catalog.clone(),
                    Arc::new(AllowingEverything),
                    clock.clone(),
                )),
                transactions,
                Arc::new(InMemoryInbox::new(clock)),
            ),
            catalog,
        }
    }

    fn ada() -> AccountId {
        AccountId::from("account_ada")
    }

    fn grace() -> AccountId {
        AccountId::from("account_grace")
    }

    fn project() -> ProjectLink {
        ProjectLink::from("project_1")
    }

    fn told(happened: &Recorded<MembersEvent>) -> Message {
        crate::message_for(happened).expect("published")
    }

    #[tokio::test]
    async fn whatever_message_comes_the_list_takes_the_project_s_members_as_they_stand_now() {
        let bench = bench();
        let id = AggregateId::from(&project());
        let mut transaction = begun().await;
        let founded = bench
            .events
            .begin(
                &mut transaction,
                &id,
                MembersCommand::Found { owner: ada() },
                &Agent::System,
            )
            .await
            .expect("founded");
        transaction
            .finish()
            .await
            .expect("finishing should succeed");
        let mut transaction = begun().await;
        bench
            .events
            .execute(
                &mut transaction,
                &id,
                MembersCommand::Join {
                    account: grace(),
                    role: Role::Reader,
                },
                &Agent::System,
            )
            .await
            .expect("joined");
        transaction
            .finish()
            .await
            .expect("finishing should succeed");

        bench
            .listener
            .handle(&told(&founded.events[0]))
            .await
            .expect("taken in");

        assert_eq!(
            bench
                .catalog
                .role_of(&project(), &grace())
                .await
                .expect("asked"),
            Some(Role::Reader),
            "the founding message comes late, yet the list holds who joined since"
        );
        assert_eq!(
            bench
                .catalog
                .role_of(&project(), &ada())
                .await
                .expect("asked"),
            Some(Role::Owner)
        );

        let mut transaction = begun().await;

        let left = bench
            .events
            .execute(
                &mut transaction,
                &id,
                MembersCommand::Leave { account: grace() },
                &Agent::System,
            )
            .await
            .expect("left");

        transaction
            .finish()
            .await
            .expect("finishing should succeed");
        bench
            .listener
            .handle(&told(&left.events[0]))
            .await
            .expect("taken in");

        assert_eq!(
            bench
                .catalog
                .role_of(&project(), &grace())
                .await
                .expect("asked"),
            None,
            "a later change replaces what the list took before"
        );
    }

    #[tokio::test]
    async fn every_change_to_members_is_heard() {
        let listener = bench().listener;

        for key in [
            members_contract::FOUNDED,
            members_contract::JOINED,
            members_contract::ROLE_CHANGED,
            members_contract::LEFT,
            members_contract::REMOVED,
        ] {
            assert!(listener.hears(&RoutingKey::parse(key).expect("a declared key")));
        }
        assert!(!listener.hears(&RoutingKey::parse("project.started").expect("a key")));
    }

    #[tokio::test]
    async fn a_message_about_members_never_founded_belongs_in_dead_letters() {
        let listener = bench().listener;
        let unheard_of = Message::opening(
            RoutingKey::parse(members_contract::JOINED).expect("a declared key"),
            json!({
                "event": { "version": 0, "name": "JOINED", "account": "account_grace", "role": "reader" },
                "aggregate": { "id": "project_9", "kind": "members", "version": 2 },
                "agent": { "kind": "system", "id": null },
                "occurred_at": "1970-01-01T00:00:00Z",
            }),
            OffsetDateTime::UNIX_EPOCH,
        );

        assert!(listener.handle(&unheard_of).await.is_err());
    }
}
