use accounts_contract::{AccountEventDTO, SIGNED_UP, SIGNED_UP_BY_INVITATION};
use async_trait::async_trait;
use eventsourcing::AccountId;
use inbox::KeptHandler;
use members_core::{MembersCore, Person};
use messaging::{Failure, Message};
use transacting::Transaction;

pub struct RememberWhoSignedUpListener {
    members: MembersCore,
}

impl RememberWhoSignedUpListener {
    pub fn new(members: MembersCore) -> Self {
        Self { members }
    }
}

#[async_trait]
impl KeptHandler for RememberWhoSignedUpListener {
    const NAME: &'static str = "remember-who-signed-up";
    const LISTENS_TO: &'static [&'static str] = &[SIGNED_UP, SIGNED_UP_BY_INVITATION];
    const WHEN_REFUSED: &'static str = "Someone who signed up is not known by name, so the projects they join list them without it.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<AccountEventDTO>(message)?;
        let (AccountEventDTO::SignedUp { personal }
        | AccountEventDTO::SignedUpByInvitation { personal }) = told.event.body
        else {
            return Err("this is no sign-up".into());
        };

        Ok(self
            .members
            .remember_person(
                Some(transaction),
                &Person {
                    account: AccountId::from(told.aggregate.id),
                    name: personal.name,
                    email: personal.email,
                },
            )
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use authorizing::AllowingEverything;
    use clock::FixedClock;
    use eventsourcing::{AccountId, Agent, InMemoryEventStore};
    use members_catalog::InMemoryMembershipCatalog;
    use members_core::MembershipCatalog;
    use messaging::{Listener, RoutingKey};
    use serde_json::{Value, json};
    use time::OffsetDateTime;
    use transacting::InMemoryTransactions;

    use inbox::{InMemoryInbox, Inboxed};

    use super::*;

    struct Bench {
        catalog: Arc<InMemoryMembershipCatalog>,
        listener: Inboxed<RememberWhoSignedUpListener>,
    }

    fn bench() -> Bench {
        let catalog = Arc::new(InMemoryMembershipCatalog::new());
        let members = MembersCore::new(
            Arc::new(InMemoryEventStore::new()),
            Arc::new(InMemoryTransactions::new()),
            catalog.clone(),
            Arc::new(AllowingEverything),
            Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        );

        Bench {
            catalog,
            listener: Inboxed::new(
                RememberWhoSignedUpListener::new(members),
                Arc::new(InMemoryTransactions::new()),
                Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(
                    OffsetDateTime::UNIX_EPOCH,
                )))),
            ),
        }
    }

    fn told(routing: &str, event: Value) -> Message {
        eventpublishing::published(
            routing,
            "account",
            "account_ada",
            1,
            event,
            &Agent::Account(AccountId::from("account_ada")),
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
            // given: the listener
            let bench = bench();
            // expect: it hears each way of signing up
            assert!(
                bench
                    .listener
                    .hears(&RoutingKey::parse(routing).expect("a declared key"))
            );

            // when: a sign-up is heard
            bench
                .listener
                .handle(&told(routing, ada_signed_up(named)))
                .await
                .expect("a sign-up is taken in");

            // then: whoever signed up is known by name and address
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
        // given: the listener
        let bench = bench();

        // expect: hearing what is no sign-up fails
        for unreadable in [
            told(SIGNED_UP, json!({ "version": 1, "name": "EMAIL_VERIFIED" })),
            told(SIGNED_UP, json!({ "version": 1, "name": "SIGNED_UP" })),
        ] {
            assert!(
                bench.listener.handle(&unreadable).await.is_err(),
                "a message the listener cannot act on belongs in dead letters"
            );
        }
        // and: nobody is known
        assert!(known(&bench).await.is_empty());
    }
}
