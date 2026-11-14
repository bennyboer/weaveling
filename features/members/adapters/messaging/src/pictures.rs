use accounts_contract::{AccountEventDTO, PICTURE_CHANGED, PICTURE_REMOVED};
use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use inbox::KeptHandler;
use members_core::MembersCore;
use messaging::{Failure, Message};
use transacting::Transaction;

pub struct RememberPicturesListener {
    members: MembersCore,
}

impl RememberPicturesListener {
    pub fn new(members: MembersCore) -> Self {
        Self { members }
    }
}

#[async_trait]
impl KeptHandler for RememberPicturesListener {
    const NAME: &'static str = "remember-pictures";
    const LISTENS_TO: &'static [&'static str] = &[PICTURE_CHANGED, PICTURE_REMOVED];
    const WHEN_REFUSED: &'static str = "The members feature did not learn that an account changed or removed its picture. Members lists show the old picture, or the initial, for that account.";

    async fn take_in(
        &self,
        transaction: &mut Transaction,
        message: &Message,
    ) -> Result<(), Failure> {
        let told = eventpublishing::published_in::<AccountEventDTO>(message)?;
        let picture = match told.event.body {
            AccountEventDTO::PictureChanged { picture, .. } => Some(picture),
            AccountEventDTO::PictureRemoved { .. } => None,
            _ => return Err("this tells neither a picture changed nor one removed".into()),
        };

        Ok(self
            .members
            .remember_picture(
                Some(transaction),
                &AccountId::from(told.aggregate.id),
                Version::of(told.aggregate.version),
                picture,
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
    use members_core::{MembershipCatalog, Pictured};
    use messaging::{Listener, RoutingKey};
    use serde_json::{Value, json};
    use time::OffsetDateTime;
    use transacting::InMemoryTransactions;

    use super::*;

    struct Bench {
        catalog: Arc<InMemoryMembershipCatalog>,
        listener: Inboxed<RememberPicturesListener>,
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
                RememberPicturesListener::new(members),
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

    fn changed(version: u64, picture: &str) -> Message {
        told(
            PICTURE_CHANGED,
            version,
            json!({
                "version": version,
                "name": "PICTURE_CHANGED",
                "picture": picture,
                "previous": null,
                "drop_previous": false,
            }),
        )
    }

    fn removed(version: u64, picture: &str) -> Message {
        told(
            PICTURE_REMOVED,
            version,
            json!({
                "version": version,
                "name": "PICTURE_REMOVED",
                "picture": picture,
                "drop": true,
            }),
        )
    }

    async fn pictures(bench: &Bench) -> Vec<Pictured> {
        bench
            .catalog
            .pictures(&[AccountId::from("account_ada")])
            .await
            .expect("asking should succeed")
    }

    fn ada_shows(picture: &str) -> Vec<Pictured> {
        vec![Pictured {
            account: AccountId::from("account_ada"),
            picture: picture.to_owned(),
        }]
    }

    #[tokio::test]
    async fn a_changed_picture_is_remembered_and_a_removed_one_forgotten() {
        // given: the listener
        let bench = bench();
        // expect: it hears both
        for routing in [PICTURE_CHANGED, PICTURE_REMOVED] {
            assert!(
                bench
                    .listener
                    .hears(&RoutingKey::parse(routing).expect("a declared key"))
            );
        }

        // when: a changed picture is heard
        bench
            .listener
            .handle(&changed(4, "asset_ada"))
            .await
            .expect("a change is taken in");

        // then: the account shows that picture
        assert_eq!(pictures(&bench).await, ada_shows("asset_ada"));
        // when: its removal is heard
        bench
            .listener
            .handle(&removed(5, "asset_ada"))
            .await
            .expect("a removal is taken in");
        // then: the account shows no picture
        assert!(pictures(&bench).await.is_empty());
    }

    #[tokio::test]
    async fn an_older_picture_heard_after_a_newer_one_changes_nothing() {
        // given: the listener, which heard a picture at the account's sixth version
        let bench = bench();
        bench
            .listener
            .handle(&changed(6, "asset_newer"))
            .await
            .expect("a change is taken in");

        // when: a change and a removal of older versions are heard late
        bench
            .listener
            .handle(&changed(4, "asset_older"))
            .await
            .expect("a late change is taken in");
        bench
            .listener
            .handle(&removed(5, "asset_older"))
            .await
            .expect("a late removal is taken in");

        // then: the account still shows the newer picture
        assert_eq!(pictures(&bench).await, ada_shows("asset_newer"));
    }

    #[tokio::test]
    async fn what_tells_no_picture_is_refused_and_teaches_nothing() {
        // given: the listener
        let bench = bench();

        // expect: hearing what is no picture event fails
        assert!(
            bench
                .listener
                .handle(&told(
                    PICTURE_CHANGED,
                    1,
                    json!({ "version": 1, "name": "EMAIL_VERIFIED" }),
                ))
                .await
                .is_err(),
            "a message the listener cannot act on belongs in dead letters"
        );
        // and: nobody shows a picture
        assert!(pictures(&bench).await.is_empty());
    }
}
