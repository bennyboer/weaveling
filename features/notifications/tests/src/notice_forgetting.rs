use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{
    AccountId, Agent, AggregateId, EventStore, InMemoryEventStore, ServiceError, Version,
};
use notifications_catalog::InMemoryNoticeCatalog;
use notifications_core::{
    About, DISMISSED_KEPT_FOR, KIND, NoticeCatalog, NoticeEvent, NoticeId, NotificationsCore,
    NotificationsError, ProjectLink, Role, UNDISMISSED_KEPT_FOR,
};
use registry::InMemoryRegistry;
use time::{Duration, OffsetDateTime};

const NOTED_AT: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

struct Kept {
    store: Arc<InMemoryEventStore<NoticeEvent>>,
    catalog: Arc<InMemoryNoticeCatalog>,
    clock: Arc<FixedClock>,
    notifications: NotificationsCore,
}

fn kept() -> Kept {
    let store = Arc::new(InMemoryEventStore::new());
    let catalog = Arc::new(InMemoryNoticeCatalog::new());
    let clock = Arc::new(FixedClock::new(NOTED_AT));

    Kept {
        notifications: NotificationsCore::new(
            store.clone(),
            catalog.clone(),
            Arc::new(InMemoryRegistry::new()),
            clock.clone(),
        ),
        store,
        catalog,
        clock,
    }
}

fn bo() -> Agent {
    Agent::Account(AccountId::from("account_bo"))
}

impl Kept {
    async fn noted(&self, cause: &str) -> NoticeId {
        let notice = self
            .notifications
            .note(
                cause,
                AccountId::from("account_bo"),
                About::RoleChanged {
                    project: ProjectLink::from("project_1"),
                    role: Role::Author,
                    by: None,
                },
            )
            .await
            .expect("noting should succeed")
            .expect("its project is not deleted");
        self.catalogued(&notice).await;

        notice
    }

    async fn catalogued(&self, notice: &NoticeId) {
        self.notifications
            .catalogue(notice)
            .await
            .expect("cataloguing should succeed");
    }

    async fn forgotten_at(&self, at: OffsetDateTime) -> usize {
        self.clock.set(at);

        self.notifications
            .forget_due(100)
            .await
            .expect("forgetting should succeed")
    }

    async fn gone(&self, notice: &NoticeId) -> bool {
        let events = self
            .store
            .read_from(&AggregateId::from(notice), KIND, Version::ZERO)
            .await
            .expect("reading should succeed");
        let read = self.notifications.read(&notice.to_string(), &bo()).await;

        events.is_empty()
            && matches!(
                read,
                Err(NotificationsError::Events(ServiceError::NotFound { .. }))
            )
    }
}

#[tokio::test]
async fn a_dismissed_notice_is_forgotten_whole_thirty_days_after_it_was_dismissed() {
    let kept = kept();
    let notice = kept.noted("members:project_1@4").await;
    let dismissed_at = NOTED_AT + Duration::days(5);
    kept.clock.set(dismissed_at);
    kept.notifications
        .dismiss(&notice.to_string(), &bo())
        .await
        .expect("dismissing should succeed");
    kept.catalogued(&notice).await;

    let early = kept
        .forgotten_at(dismissed_at + DISMISSED_KEPT_FOR - Duration::seconds(1))
        .await;
    let due = kept
        .forgotten_at(dismissed_at + DISMISSED_KEPT_FOR + Duration::seconds(1))
        .await;

    assert_eq!((early, due), (0, 1), "a day short of thirty is kept");
    assert!(kept.gone(&notice).await, "its events go with it");
    assert!(
        kept.catalog
            .due_for_forgetting(
                dismissed_at + DISMISSED_KEPT_FOR * 2,
                dismissed_at + DISMISSED_KEPT_FOR * 2,
                10
            )
            .await
            .expect("asking should succeed")
            .is_empty(),
        "and its row in the list"
    );
}

#[tokio::test]
async fn a_notice_never_dismissed_is_forgotten_ninety_days_after_it_was_noted() {
    let kept = kept();
    let notice = kept.noted("members:project_1@4").await;

    let early = kept
        .forgotten_at(NOTED_AT + UNDISMISSED_KEPT_FOR - Duration::seconds(1))
        .await;
    let listed = kept
        .notifications
        .open_for(&bo())
        .await
        .expect("listing should succeed")
        .len();
    let due = kept
        .forgotten_at(NOTED_AT + UNDISMISSED_KEPT_FOR + Duration::seconds(1))
        .await;

    assert_eq!((early, listed, due), (0, 1, 1));
    assert!(kept.gone(&notice).await);
    assert!(
        kept.notifications
            .open_for(&bo())
            .await
            .expect("listing should succeed")
            .is_empty()
    );
}

#[tokio::test]
async fn a_notice_of_a_deleted_project_is_forgotten_at_once() {
    let kept = kept();
    let notice = kept.noted("members:project_1@4").await;
    kept.notifications
        .lay_project_tombstone(&ProjectLink::from("project_1"), NOTED_AT)
        .await
        .expect("laying a tombstone should succeed");

    assert_eq!(kept.forgotten_at(NOTED_AT + Duration::hours(1)).await, 1);
    assert!(kept.gone(&notice).await);
}

#[test]
fn notices_are_kept_thirty_days_dismissed_and_ninety_not() {
    assert_eq!(DISMISSED_KEPT_FOR, Duration::days(30));
    assert_eq!(UNDISMISSED_KEPT_FOR, Duration::days(90));
}
