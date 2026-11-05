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
use time::{Duration, OffsetDateTime};
use transacting::InMemoryTransactions;

const NOTED_AT: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

struct Kept {
    store: Arc<InMemoryEventStore<NoticeEvent>>,
    catalog: Arc<InMemoryNoticeCatalog>,
    clock: Arc<FixedClock>,
    notifications: NotificationsCore,
}

fn a_workbench() -> Kept {
    let store = Arc::new(InMemoryEventStore::new());
    let catalog = Arc::new(InMemoryNoticeCatalog::new());
    let clock = Arc::new(FixedClock::new(NOTED_AT));

    Kept {
        notifications: NotificationsCore::new(
            store.clone(),
            Arc::new(InMemoryTransactions::new()),
            catalog.clone(),
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
    async fn a_listed_notice(&self) -> NoticeId {
        let notice = self
            .notifications
            .note(
                None,
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
            .catalogue(None, notice)
            .await
            .expect("cataloguing should succeed");
    }

    async fn forgetting_what_is_due_at(&self, at: OffsetDateTime) -> usize {
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
    // given: a listed notice dismissed five days after it was noted
    let kept = a_workbench();
    let notice = kept.a_listed_notice().await;
    let dismissed_at = NOTED_AT + Duration::days(5);
    kept.clock.set(dismissed_at);
    kept.notifications
        .dismiss(&notice.to_string(), None, &bo())
        .await
        .expect("dismissing should succeed");
    kept.catalogued(&notice).await;

    // when: what is due is forgotten a moment before thirty days are up, and a moment after
    let early = kept
        .forgetting_what_is_due_at(dismissed_at + DISMISSED_KEPT_FOR - Duration::seconds(1))
        .await;
    let due = kept
        .forgetting_what_is_due_at(dismissed_at + DISMISSED_KEPT_FOR + Duration::seconds(1))
        .await;

    // then: it is forgotten only after
    assert_eq!((early, due), (0, 1), "a day short of thirty is kept");
    // and: its events go with it
    assert!(kept.gone(&notice).await, "its events go with it");
    // and: its row in the list
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
    // given: a listed notice never dismissed
    let kept = a_workbench();
    let notice = kept.a_listed_notice().await;

    // when: what is due is forgotten a moment before ninety days are up, the list read, and forgotten a moment after
    let early = kept
        .forgetting_what_is_due_at(NOTED_AT + UNDISMISSED_KEPT_FOR - Duration::seconds(1))
        .await;
    let listed = kept
        .notifications
        .open_for(&bo())
        .await
        .expect("listing should succeed")
        .len();
    let due = kept
        .forgetting_what_is_due_at(NOTED_AT + UNDISMISSED_KEPT_FOR + Duration::seconds(1))
        .await;

    // then: it is listed until then and forgotten after
    assert_eq!((early, listed, due), (0, 1, 1));
    // and: its events go with it
    assert!(kept.gone(&notice).await);
    // and: it is listed no more
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
    // given: a listed notice of a deleted project
    let kept = a_workbench();
    let notice = kept.a_listed_notice().await;
    kept.notifications
        .lay_project_tombstone(None, &ProjectLink::from("project_1"), NOTED_AT)
        .await
        .expect("laying a tombstone should succeed");

    // then: forgetting what is due forgets it at once
    assert_eq!(
        kept.forgetting_what_is_due_at(NOTED_AT + Duration::hours(1))
            .await,
        1
    );
    // and: its events go with it
    assert!(kept.gone(&notice).await);
}

#[test]
fn notices_are_kept_thirty_days_dismissed_and_ninety_not() {
    // expect: notices are kept thirty days dismissed and ninety not
    assert_eq!(DISMISSED_KEPT_FOR, Duration::days(30));
    assert_eq!(UNDISMISSED_KEPT_FOR, Duration::days(90));
}

#[tokio::test]
async fn a_project_tombstone_forgotten_is_forgotten_for_good() {
    // given: a project's tombstone
    let kept = a_workbench();
    kept.notifications
        .lay_project_tombstone(None, &ProjectLink::from("project_1"), NOTED_AT)
        .await
        .expect("laying a tombstone should succeed");

    // when: what was laid before a later moment is forgotten twice
    let forgotten = [
        kept.notifications
            .forget_tombstones_laid_before(NOTED_AT + Duration::seconds(1), 10)
            .await
            .expect("forgetting should succeed"),
        kept.notifications
            .forget_tombstones_laid_before(NOTED_AT + Duration::seconds(1), 10)
            .await
            .expect("forgetting should succeed"),
    ];

    // then: it is forgotten once
    assert_eq!(forgotten, [1, 0]);
}

#[tokio::test]
async fn a_notice_catalogued_after_it_was_forgotten_is_nothing_to_catalogue() {
    // given: a forgotten notice
    let kept = a_workbench();
    let notice = kept.a_listed_notice().await;
    kept.forgetting_what_is_due_at(NOTED_AT + UNDISMISSED_KEPT_FOR + Duration::seconds(1))
        .await;

    // when: it is catalogued
    kept.catalogued(&notice).await;

    // then: nothing is listed
    assert!(
        kept.catalog
            .open_for(&AccountId::from("account_bo"))
            .await
            .expect("listing should succeed")
            .is_empty(),
        "a message about it still on its way finds nothing to catalogue, which is no failure"
    );
}

#[tokio::test]
async fn a_forgotten_notice_s_tombstone_is_forgotten_by_the_same_chore() {
    // given: a forgotten notice
    let kept = a_workbench();
    kept.a_listed_notice().await;
    let forgotten_at = NOTED_AT + UNDISMISSED_KEPT_FOR + Duration::seconds(1);
    kept.forgetting_what_is_due_at(forgotten_at).await;

    // when: what was laid before a later moment is forgotten twice
    let forgotten = [
        kept.notifications
            .forget_tombstones_laid_before(forgotten_at + Duration::seconds(1), 10)
            .await
            .expect("forgetting should succeed"),
        kept.notifications
            .forget_tombstones_laid_before(forgotten_at + Duration::seconds(1), 10)
            .await
            .expect("forgetting should succeed"),
    ];

    // then: its tombstone is forgotten once
    assert_eq!(forgotten, [1, 0]);
}
