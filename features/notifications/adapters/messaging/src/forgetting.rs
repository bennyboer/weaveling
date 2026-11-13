use accounts_contract::{AccountEventDTO, DELETED};
use async_trait::async_trait;
use eventsourcing::AccountId;
use inbox::KeptHandler;
use messaging::{Failure, Message};
use notifications_core::NotificationsCore;
use transacting::Transaction;

pub struct ForgetNoticesOfDeletedAccountListener {
    notifications: NotificationsCore,
}

impl ForgetNoticesOfDeletedAccountListener {
    pub fn new(notifications: NotificationsCore) -> Self {
        Self { notifications }
    }
}

#[async_trait]
impl KeptHandler for ForgetNoticesOfDeletedAccountListener {
    const NAME: &'static str = "forget-notices-of-deleted-account";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str = "The notices of a deleted account were not deleted. They and the account's name stay stored until the notices are deleted for being old.";

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
            .notifications
            .forget_account(Some(transaction), &AccountId::from(told.aggregate.id))
            .await?)
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use clock::FixedClock;
    use eventsourcing::{Agent, AggregateId, EventStore, InMemoryEventStore, Version};
    use inbox::{InMemoryInbox, Inboxed};
    use messaging::{Listener, RoutingKey};
    use notifications_catalog::InMemoryNoticeCatalog;
    use notifications_core::{
        About, KIND, NoticeCatalog, NoticeEvent, NoticeId, Person, ProjectLink, Role,
    };
    use serde_json::{Value, json};
    use time::OffsetDateTime;
    use transacting::InMemoryTransactions;

    use super::*;

    struct Bench {
        store: Arc<InMemoryEventStore<NoticeEvent>>,
        catalog: Arc<InMemoryNoticeCatalog>,
        notifications: NotificationsCore,
        listener: Inboxed<ForgetNoticesOfDeletedAccountListener>,
    }

    fn a_workbench() -> Bench {
        let clock = Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH));
        let store = Arc::new(InMemoryEventStore::new());
        let catalog = Arc::new(InMemoryNoticeCatalog::new());
        let transactions = Arc::new(InMemoryTransactions::new());
        let notifications = NotificationsCore::new(
            store.clone(),
            transactions.clone(),
            catalog.clone(),
            clock.clone(),
        );

        Bench {
            listener: Inboxed::new(
                ForgetNoticesOfDeletedAccountListener::new(notifications.clone()),
                transactions,
                Arc::new(InMemoryInbox::new(clock)),
            ),
            notifications,
            store,
            catalog,
        }
    }

    fn deleted(of: &str, body: Value) -> Message {
        Message::opening(
            RoutingKey::parse(DELETED).expect("a declared key"),
            json!({
                "event": body,
                "aggregate": { "id": of, "kind": "account", "version": 9 },
                "agent": { "kind": "system" },
                "occurred_at": "1970-01-01T00:00:00Z",
            }),
            OffsetDateTime::UNIX_EPOCH,
        )
    }

    impl Bench {
        async fn a_listed_notice_for(&self, account: &str) -> NoticeId {
            let notice = self
                .notifications
                .note(
                    None,
                    AccountId::from(account),
                    About::RoleChanged {
                        project: ProjectLink::from("project_1"),
                        role: Role::Author,
                        by: None,
                    },
                )
                .await
                .expect("noting should succeed")
                .expect("its project is not deleted");
            self.notifications
                .catalogue(None, &notice)
                .await
                .expect("cataloguing should succeed");

            notice
        }

        async fn named(&self, account: &str, name: &str) {
            self.notifications
                .remember_person(
                    None,
                    &Person {
                        account: AccountId::from(account),
                        name: name.to_owned(),
                    },
                )
                .await
                .expect("remembering should succeed");
        }

        async fn events_of(&self, notice: &NoticeId) -> usize {
            self.store
                .read_from(&AggregateId::from(notice), KIND, Version::ZERO)
                .await
                .expect("reading should succeed")
                .len()
        }
    }

    #[tokio::test]
    async fn a_deleted_account_s_notices_and_name_are_forgotten_whole_and_no_other_s() {
        // given: the listener
        let bench = a_workbench();
        // expect: it hears an account deleted
        assert!(
            bench
                .listener
                .hears(&RoutingKey::parse(DELETED).expect("a declared key"))
        );
        // given: two notices for bo, one of them dismissed, and one for cy
        let open = bench.a_listed_notice_for("account_bo").await;
        let dismissed = bench.a_listed_notice_for("account_bo").await;
        bench
            .notifications
            .dismiss(
                &dismissed.to_string(),
                None,
                &Agent::Account(AccountId::from("account_bo")),
            )
            .await
            .expect("dismissing should succeed");
        bench
            .notifications
            .catalogue(None, &dismissed)
            .await
            .expect("cataloguing should succeed");
        let cy_s = bench.a_listed_notice_for("account_cy").await;
        // and: both named
        bench.named("account_bo", "Bo").await;
        bench.named("account_cy", "Cy").await;

        // when: bo's account is heard deleted, twice
        for _ in 0..2 {
            bench
                .listener
                .handle(&deleted(
                    "account_bo",
                    json!({ "version": 9, "name": "DELETED" }),
                ))
                .await
                .expect("messages come at least once");
        }

        // then: bo's notices are gone, their events with them
        assert_eq!(
            (
                bench.events_of(&open).await,
                bench.events_of(&dismissed).await
            ),
            (0, 0)
        );
        assert!(
            bench
                .catalog
                .notices_of(&AccountId::from("account_bo"))
                .await
                .expect("asking should succeed")
                .is_empty()
        );
        // and: a late word of one does not list it again
        bench
            .notifications
            .catalogue(None, &open)
            .await
            .expect("cataloguing what is gone is no failure");
        assert!(
            bench
                .catalog
                .notices_of(&AccountId::from("account_bo"))
                .await
                .expect("asking should succeed")
                .is_empty()
        );
        // and: cy's notice and name stand, bo's name is gone
        assert_eq!(bench.events_of(&cy_s).await, 1);
        assert_eq!(
            bench
                .catalog
                .people(&[AccountId::from("account_bo"), AccountId::from("account_cy")])
                .await
                .expect("asking should succeed"),
            vec![Person {
                account: AccountId::from("account_cy"),
                name: "Cy".to_owned(),
            }]
        );
    }

    #[tokio::test]
    async fn what_tells_no_deletion_is_refused() {
        // given: the listener
        let bench = a_workbench();

        // expect: hearing what is no deletion fails
        assert!(
            bench
                .listener
                .handle(&deleted(
                    "account_bo",
                    json!({ "version": 9, "name": "EMAIL_VERIFIED" })
                ))
                .await
                .is_err(),
            "a message the listener cannot act on belongs in dead letters"
        );
    }
}
