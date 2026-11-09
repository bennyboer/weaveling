use accounts_contract::{AccountEventDTO, DELETION_ASKED, DELETION_CALLED_OFF};
use async_trait::async_trait;
use clock::text;
use eventsourcing::{AccountId, Version};
use inbox::KeptHandler;
use members_core::MembersCore;
use messaging::{Failure, Message};
use transacting::Transaction;

pub struct RememberWhoIsLeavingListener {
    members: MembersCore,
}

impl RememberWhoIsLeavingListener {
    pub fn new(members: MembersCore) -> Self {
        Self { members }
    }
}

#[async_trait]
impl KeptHandler for RememberWhoIsLeavingListener {
    const NAME: &'static str = "remember-who-is-leaving";
    const LISTENS_TO: &'static [&'static str] = &[DELETION_ASKED, DELETION_CALLED_OFF];
    const WHEN_REFUSED: &'static str = "An account asked to be deleted, or kept after all, did not reach the members, so its role may still be changed, or stays frozen, as it should not.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<AccountEventDTO>(message)?;
        let until = match told.event.body {
            AccountEventDTO::DeletionAsked { until, .. } => {
                Some(text::read(&until).ok_or("the day the account is kept until is no day")?)
            }
            AccountEventDTO::DeletionCalledOff => None,
            _ => return Err("this tells neither a deletion asked nor one called off".into()),
        };

        Ok(self
            .members
            .remember_leaving(
                Some(transaction),
                &AccountId::from(told.aggregate.id),
                Version::of(told.aggregate.version),
                until,
            )
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use authorizing::AllowingEverything;
    use clock::FixedClock;
    use eventsourcing::{Agent, InMemoryEventStore};
    use inbox::{InMemoryInbox, Inboxed};
    use members_catalog::InMemoryMembershipCatalog;
    use members_core::{Leaving, MembershipCatalog};
    use messaging::{Listener, RoutingKey};
    use serde_json::{Value, json};
    use time::OffsetDateTime;
    use transacting::InMemoryTransactions;

    use super::*;

    struct Bench {
        catalog: Arc<InMemoryMembershipCatalog>,
        listener: Inboxed<RememberWhoIsLeavingListener>,
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
                RememberWhoIsLeavingListener::new(members),
                Arc::new(InMemoryTransactions::new()),
                Arc::new(InMemoryInbox::new(Arc::new(FixedClock::new(
                    OffsetDateTime::UNIX_EPOCH,
                )))),
            ),
        }
    }

    fn told(routing: &str, version: u64, event: Value) -> Message {
        eventpublishing::published(
            routing,
            "account",
            "account_ada",
            version,
            event,
            &Agent::Account(AccountId::from("account_ada")),
            OffsetDateTime::UNIX_EPOCH,
        )
    }

    async fn leaving(bench: &Bench) -> Vec<Leaving> {
        bench
            .catalog
            .leaving(&[AccountId::from("account_ada")])
            .await
            .expect("asking should succeed")
    }

    #[tokio::test]
    async fn an_account_asked_to_be_deleted_is_leaving_until_then_and_no_longer_once_kept() {
        // given: the listener
        let bench = bench();
        // expect: it hears both
        for routing in [DELETION_ASKED, DELETION_CALLED_OFF] {
            assert!(
                bench
                    .listener
                    .hears(&RoutingKey::parse(routing).expect("a declared key"))
            );
        }

        // when: a deletion is heard asked
        bench
            .listener
            .handle(&told(
                DELETION_ASKED,
                7,
                json!({
                    "version": 7,
                    "name": "DELETION_ASKED",
                    "until": "2026-12-09T10:00:00Z",
                    "by": "holder",
                }),
            ))
            .await
            .expect("an asking is taken in");

        // then: the account is leaving until that day
        assert_eq!(
            leaving(&bench).await,
            vec![Leaving {
                account: AccountId::from("account_ada"),
                until: text::read("2026-12-09T10:00:00Z").expect("a day"),
            }]
        );
        // when: it is heard called off
        bench
            .listener
            .handle(&told(
                DELETION_CALLED_OFF,
                8,
                json!({ "version": 8, "name": "DELETION_CALLED_OFF" }),
            ))
            .await
            .expect("a calling off is taken in");
        // then: it is leaving no longer
        assert!(leaving(&bench).await.is_empty());
    }

    #[tokio::test]
    async fn what_tells_no_deletion_is_refused_and_teaches_nothing() {
        // given: the listener
        let bench = bench();

        // expect: hearing what is no deletion, or a day that is none, fails
        for unreadable in [
            told(
                DELETION_ASKED,
                1,
                json!({ "version": 1, "name": "EMAIL_VERIFIED" }),
            ),
            told(
                DELETION_ASKED,
                1,
                json!({ "version": 1, "name": "DELETION_ASKED", "until": "soon", "by": "holder" }),
            ),
        ] {
            assert!(
                bench.listener.handle(&unreadable).await.is_err(),
                "a message the listener cannot act on belongs in dead letters"
            );
        }
        // and: nobody is leaving
        assert!(leaving(&bench).await.is_empty());
    }
}
