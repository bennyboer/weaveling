use std::sync::Arc;

use async_trait::async_trait;
use clock::Clock;
use eventsourcing::{AggregateId, EventSourcingService, EventStore};
use members_contract::EVERY_MEMBERS;
use members_core::{Members, MembersEvent, MembershipCatalog, ProjectLink, ProjectMembers};
use messaging::{Failure, Handler, Message};

pub struct CatalogueMembersListener {
    events: EventSourcingService<Members>,
    catalog: Arc<dyn MembershipCatalog>,
}

impl CatalogueMembersListener {
    pub fn new(
        store: Arc<dyn EventStore<MembersEvent>>,
        catalog: Arc<dyn MembershipCatalog>,
        clock: Arc<dyn Clock>,
    ) -> Self {
        Self {
            events: EventSourcingService::new(store, clock),
            catalog,
        }
    }
}

#[async_trait]
impl Handler for CatalogueMembersListener {
    const NAME: &'static str = "catalogue-members";
    const LISTENS_TO: &'static [&'static str] = &[EVERY_MEMBERS];
    const WHEN_REFUSED: &'static str =
        "A change to a project's members did not reach the list of who may work where.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let project = eventpublishing::aggregate_in::<ProjectLink>(message)?;
        let standing = self.events.latest(&AggregateId::from(&project)).await?;

        Ok(self
            .catalog
            .remember(&ProjectMembers::of(
                project,
                standing.version,
                &standing.state,
            ))
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use clock::FixedClock;
    use eventsourcing::{AccountId, Agent, Recorded};
    use members_catalog::InMemoryMembershipCatalog;
    use members_core::{MembersCommand, Role};
    use messaging::{Listener, RoutingKey};
    use serde_json::json;
    use time::OffsetDateTime;

    use super::*;

    struct Bench {
        events: EventSourcingService<Members>,
        catalog: Arc<InMemoryMembershipCatalog>,
        listener: CatalogueMembersListener,
    }

    fn bench() -> Bench {
        let store: Arc<dyn EventStore<MembersEvent>> =
            Arc::new(eventsourcing::InMemoryEventStore::new());
        let clock: Arc<dyn Clock> = Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH));
        let catalog = Arc::new(InMemoryMembershipCatalog::new());

        Bench {
            events: EventSourcingService::new(store.clone(), clock.clone()),
            listener: CatalogueMembersListener::new(store, catalog.clone(), clock),
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
        let founded = bench
            .events
            .begin(&id, MembersCommand::Found { owner: ada() }, &Agent::System)
            .await
            .expect("founded");
        bench
            .events
            .execute(
                &id,
                MembersCommand::Join {
                    account: grace(),
                    role: Role::Reader,
                },
                &Agent::System,
            )
            .await
            .expect("joined");

        bench
            .listener
            .take_in(&told(&founded.events[0]))
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

        let left = bench
            .events
            .execute(
                &id,
                MembersCommand::Leave { account: grace() },
                &Agent::System,
            )
            .await
            .expect("left");
        bench
            .listener
            .take_in(&told(&left.events[0]))
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

        assert!(listener.take_in(&unheard_of).await.is_err());
    }
}
