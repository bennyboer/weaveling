use accounts_contract::{AccountEventDTO, SIGNED_UP, SIGNED_UP_BY_INVITATION};
use async_trait::async_trait;
use eventsourcing::AccountId;
use members_core::{MembersCore, Person};
use messaging::{Failure, Handler, Message};

pub struct RememberWhoSignedUpListener {
    members: MembersCore,
}

impl RememberWhoSignedUpListener {
    pub fn new(members: MembersCore) -> Self {
        Self { members }
    }
}

#[async_trait]
impl Handler for RememberWhoSignedUpListener {
    const NAME: &'static str = "remember-who-signed-up";
    const LISTENS_TO: &'static [&'static str] = &[SIGNED_UP, SIGNED_UP_BY_INVITATION];
    const WHEN_REFUSED: &'static str = "Someone who signed up is not known by name, so the projects they join list them without it.";

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<AccountEventDTO>(message)?;
        let (AccountEventDTO::SignedUp { personal }
        | AccountEventDTO::SignedUpByInvitation { personal }) = told.event.body
        else {
            return Err("this is no sign-up".into());
        };

        Ok(self
            .members
            .remember_person(&Person {
                account: AccountId::from(told.aggregate.id),
                name: personal.name,
                email: personal.email,
            })
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use authorizing::AllowingEverything;
    use clock::FixedClock;
    use eventsourcing::InMemoryEventStore;
    use members_catalog::InMemoryMembershipCatalog;
    use members_core::MembershipCatalog;
    use messaging::{Listener, RoutingKey};
    use serde_json::{Value, json};
    use time::OffsetDateTime;

    use super::*;

    struct Bench {
        catalog: Arc<InMemoryMembershipCatalog>,
        listener: RememberWhoSignedUpListener,
    }

    fn bench() -> Bench {
        let catalog = Arc::new(InMemoryMembershipCatalog::new());
        let members = MembersCore::new(
            Arc::new(InMemoryEventStore::new()),
            catalog.clone(),
            Arc::new(AllowingEverything),
            Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        );

        Bench {
            catalog,
            listener: RememberWhoSignedUpListener::new(members),
        }
    }

    fn told(routing: &str, event: Value) -> Message {
        Message::opening(
            RoutingKey::parse(routing).expect("a declared key"),
            json!({
                "event": event,
                "aggregate": { "id": "account_ada", "kind": "account", "version": 1 },
                "agent": { "kind": "account", "id": "account_ada" },
                "occurred_at": "1970-01-01T00:00:00Z",
            }),
            OffsetDateTime::UNIX_EPOCH,
        )
    }

    fn ada_signed_up(named: &str) -> Value {
        json!({
            "version": 1,
            "name": named,
            "personal": { "name": "Ada Lovelace", "email": "ada@example.com" },
        })
    }

    async fn known(bench: &Bench) -> Vec<Person> {
        bench
            .catalog
            .people(&[AccountId::from("account_ada")])
            .await
            .expect("asking should succeed")
    }

    #[tokio::test]
    async fn whoever_signed_up_either_way_is_known_by_name_and_address() {
        for (routing, named) in [
            (SIGNED_UP, "SIGNED_UP"),
            (SIGNED_UP_BY_INVITATION, "SIGNED_UP_BY_INVITATION"),
        ] {
            let bench = bench();
            assert!(
                bench
                    .listener
                    .hears(&RoutingKey::parse(routing).expect("a declared key"))
            );

            bench
                .listener
                .take_in(&told(routing, ada_signed_up(named)))
                .await
                .expect("a sign-up is taken in");

            assert_eq!(
                known(&bench).await,
                vec![Person {
                    account: AccountId::from("account_ada"),
                    name: "Ada Lovelace".to_owned(),
                    email: "ada@example.com".to_owned(),
                }]
            );
        }
    }

    #[tokio::test]
    async fn what_is_no_sign_up_is_refused_and_teaches_nothing() {
        let bench = bench();

        for unreadable in [
            told(SIGNED_UP, json!({ "version": 1, "name": "EMAIL_VERIFIED" })),
            told(SIGNED_UP, json!({ "version": 1, "name": "SIGNED_UP" })),
        ] {
            assert!(
                bench.listener.take_in(&unreadable).await.is_err(),
                "a message the listener cannot act on belongs in dead letters"
            );
        }
        assert!(known(&bench).await.is_empty());
    }
}
