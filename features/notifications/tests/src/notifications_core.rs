use std::fmt::Debug;
use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{
    AccountId, Agent, AggregateId, EventStore, FailsAppending, InMemoryEventStore, ServiceError,
    StoreError, Version,
};
use notifications_catalog::InMemoryNoticeCatalog;
use notifications_core::{
    About, KIND, NoticeEvent, NoticeId, NotificationsCore, NotificationsError, ProjectLink, Role,
};
use time::OffsetDateTime;
use transacting::InMemoryTransactions;

struct Kept {
    store: Arc<InMemoryEventStore<NoticeEvent>>,
    notifications: NotificationsCore,
}

fn a_workbench() -> Kept {
    let store = Arc::new(InMemoryEventStore::new());

    Kept {
        notifications: NotificationsCore::new(
            store.clone(),
            Arc::new(InMemoryTransactions::new()),
            Arc::new(InMemoryNoticeCatalog::new()),
            Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        ),
        store,
    }
}

fn an_account(id: &str) -> Agent {
    Agent::Account(AccountId::from(id))
}

fn made_an_author() -> About {
    About::RoleChanged {
        project: ProjectLink::from("project_1"),
        role: Role::Author,
        by: Some(AccountId::from("account_ada")),
    }
}

async fn a_notice_for_bo(kept: &Kept) -> NoticeId {
    kept.notifications
        .note(None, AccountId::from("account_bo"), made_an_author())
        .await
        .expect("noting should succeed")
        .expect("its project is not deleted")
}

async fn events_of(kept: &Kept, notice: &NoticeId) -> Vec<NoticeEvent> {
    kept.store
        .read_from(&AggregateId::from(notice), KIND, Version::of(1))
        .await
        .expect("reading should succeed")
        .into_iter()
        .map(|recorded| recorded.event)
        .collect()
}

fn not_found(answer: Result<impl Debug, NotificationsError>) -> bool {
    matches!(
        answer,
        Err(NotificationsError::Events(ServiceError::NotFound { .. }))
    )
}

#[tokio::test]
async fn a_notice_is_noted_and_read_by_whom_it_is_for() {
    // given: nothing noted yet
    let kept = a_workbench();

    // when: a notice is noted for bo
    let first = a_notice_for_bo(&kept).await;

    // then: bo reads it, not dismissed
    let read = kept
        .notifications
        .read(&first.to_string(), &an_account("account_bo"))
        .await
        .expect("bo reads their own notice");
    assert_eq!(read.state.about(), &made_an_author());
    assert!(!read.state.is_dismissed());
    // and: it is kept as noted for bo
    assert_eq!(
        events_of(&kept, &first).await,
        vec![NoticeEvent::Noted {
            account: AccountId::from("account_bo"),
            about: made_an_author(),
        }]
    );
}

#[tokio::test]
async fn another_s_notice_is_not_there_to_read_or_dismiss() {
    // given: a notice for bo
    let kept = a_workbench();
    let notice = a_notice_for_bo(&kept).await.to_string();

    // then: nobody else may read or dismiss it
    for agent in [an_account("account_ada"), Agent::System, Agent::Local] {
        assert!(not_found(kept.notifications.read(&notice, &agent).await));
        assert!(not_found(kept.notifications.dismiss(&notice, &agent).await));
    }
    // and: it stands undismissed for bo
    assert!(
        !kept
            .notifications
            .read(&notice, &an_account("account_bo"))
            .await
            .expect("still there for bo")
            .state
            .is_dismissed()
    );
}

#[tokio::test]
async fn whom_it_is_for_dismisses_it_once() {
    // given: a notice for bo
    let kept = a_workbench();
    let notice = a_notice_for_bo(&kept).await;

    // when: bo dismisses it twice
    for _ in 0..2 {
        kept.notifications
            .dismiss(&notice.to_string(), &an_account("account_bo"))
            .await
            .expect("dismissing should succeed");
    }

    // then: it is dismissed
    assert!(
        kept.notifications
            .read(&notice.to_string(), &an_account("account_bo"))
            .await
            .expect("still there to read")
            .state
            .is_dismissed()
    );
    // and: once
    assert_eq!(
        events_of(&kept, &notice).await.last(),
        Some(&NoticeEvent::Dismissed)
    );
    assert_eq!(events_of(&kept, &notice).await.len(), 2, "dismissed once");
}

#[tokio::test]
async fn a_notice_nobody_noted_or_no_id_at_all_is_not_there() {
    // given: nothing noted yet
    let kept = a_workbench();

    // then: dismissing a notice nobody noted finds none
    assert!(not_found(
        kept.notifications
            .dismiss(
                &NoticeId::generate(OffsetDateTime::UNIX_EPOCH).to_string(),
                &an_account("account_bo"),
            )
            .await
    ));
    // and: dismissing what is no id at all is refused as invalid
    assert!(matches!(
        kept.notifications
            .dismiss("no notice", &an_account("account_bo"))
            .await,
        Err(NotificationsError::InvalidId(_))
    ));
}

async fn dismissing_while_losing_races(losses: u8) -> (Result<(), NotificationsError>, usize) {
    let kept = a_workbench();
    let notice = a_notice_for_bo(&kept).await;
    let losing = NotificationsCore::new(
        Arc::new(FailsAppending::losing_races(kept.store.clone(), losses)),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryNoticeCatalog::new()),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );

    let dismissed = losing
        .dismiss(&notice.to_string(), &an_account("account_bo"))
        .await;

    (dismissed, events_of(&kept, &notice).await.len())
}

#[tokio::test]
async fn a_dismissal_that_lost_a_race_is_tried_again_and_lands() {
    // when: a dismissal loses four races
    let (dismissed, told) = dismissing_while_losing_races(4).await;

    // then: it lands
    dismissed.expect("the fifth try lands");
    // and: once
    assert_eq!(told, 2, "dismissed once");
}

#[tokio::test]
async fn a_dismissal_that_lost_every_race_it_may_try_is_a_conflict() {
    // when: a dismissal loses five races
    let (dismissed, told) = dismissing_while_losing_races(5).await;

    // then: it is a conflict
    assert!(matches!(
        dismissed,
        Err(NotificationsError::Events(ServiceError::Store(
            StoreError::Outdated { .. }
        )))
    ));
    // and: nothing is dismissed
    assert_eq!(told, 1, "noted, never dismissed");
}
