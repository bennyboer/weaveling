use std::fmt::Debug;
use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{
    AccountId, Agent, AggregateId, EventStore, InMemoryEventStore, ServiceError, Version,
};
use notifications_core::{
    About, KIND, NoticeEvent, NoticeId, NotificationsCore, NotificationsError, ProjectLink, Role,
};
use registry::InMemoryRegistry;
use time::OffsetDateTime;

struct Kept {
    store: Arc<InMemoryEventStore<NoticeEvent>>,
    notifications: NotificationsCore,
}

fn kept() -> Kept {
    let store = Arc::new(InMemoryEventStore::new());

    Kept {
        notifications: NotificationsCore::new(
            store.clone(),
            Arc::new(InMemoryRegistry::new()),
            Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        ),
        store,
    }
}

fn account(id: &str) -> Agent {
    Agent::Account(AccountId::from(id))
}

fn made_an_author() -> About {
    About::RoleChanged {
        project: ProjectLink::from("project_1"),
        role: Role::Author,
        by: Some(AccountId::from("account_ada")),
    }
}

async fn noted_for_bo(kept: &Kept) -> NoticeId {
    kept.notifications
        .note(
            "members:project_1@4",
            AccountId::from("account_bo"),
            made_an_author(),
        )
        .await
        .expect("noting should succeed")
}

async fn told(kept: &Kept, notice: &NoticeId) -> Vec<NoticeEvent> {
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
async fn a_notice_is_noted_once_for_its_cause_and_read_by_whom_it_is_for() {
    let kept = kept();

    let first = noted_for_bo(&kept).await;
    let again = noted_for_bo(&kept).await;

    assert_eq!(first, again, "a cause heard twice is one notice");
    let read = kept
        .notifications
        .read(&first.to_string(), &account("account_bo"))
        .await
        .expect("bo reads their own notice");
    assert_eq!(read.state.about(), &made_an_author());
    assert!(!read.state.is_dismissed());
    assert_eq!(
        told(&kept, &first).await,
        vec![NoticeEvent::Noted {
            account: AccountId::from("account_bo"),
            about: made_an_author(),
        }]
    );
}

#[tokio::test]
async fn another_s_notice_is_not_there_to_read_or_dismiss() {
    let kept = kept();
    let notice = noted_for_bo(&kept).await.to_string();

    for agent in [account("account_ada"), Agent::System, Agent::Local] {
        assert!(not_found(kept.notifications.read(&notice, &agent).await));
        assert!(not_found(kept.notifications.dismiss(&notice, &agent).await));
    }
    assert!(
        !kept
            .notifications
            .read(&notice, &account("account_bo"))
            .await
            .expect("still there for bo")
            .state
            .is_dismissed()
    );
}

#[tokio::test]
async fn whom_it_is_for_dismisses_it_once() {
    let kept = kept();
    let notice = noted_for_bo(&kept).await;

    for _ in 0..2 {
        kept.notifications
            .dismiss(&notice.to_string(), &account("account_bo"))
            .await
            .expect("dismissing should succeed");
    }

    assert!(
        kept.notifications
            .read(&notice.to_string(), &account("account_bo"))
            .await
            .expect("still there to read")
            .state
            .is_dismissed()
    );
    assert_eq!(
        told(&kept, &notice).await.last(),
        Some(&NoticeEvent::Dismissed)
    );
    assert_eq!(told(&kept, &notice).await.len(), 2, "dismissed once");
}

#[tokio::test]
async fn a_notice_nobody_noted_or_no_id_at_all_is_not_there() {
    let kept = kept();

    assert!(not_found(
        kept.notifications
            .dismiss(
                &NoticeId::generate(OffsetDateTime::UNIX_EPOCH).to_string(),
                &account("account_bo"),
            )
            .await
    ));
    assert!(matches!(
        kept.notifications
            .dismiss("no notice", &account("account_bo"))
            .await,
        Err(NotificationsError::InvalidId(_))
    ));
}
