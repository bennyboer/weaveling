use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{AccountId, Agent, AggregateId, EventStore, InMemoryEventStore, Version};
use messaging::{Handler, Listener, RoutingKey};
use notifications_catalog::InMemoryNoticeCatalog;
use notifications_contract::{DISMISSED, NOTED};
use notifications_core::{
    About, KIND, NoticeEvent, NoticeId, NotificationsCore, ProjectLink, Role,
};
use notifications_messaging::CatalogueNoticeListener;
use registry::InMemoryRegistry;
use time::{Duration, OffsetDateTime};

const NOTED_AT: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

struct Listed {
    store: Arc<InMemoryEventStore<NoticeEvent>>,
    clock: Arc<FixedClock>,
    notifications: NotificationsCore,
    listener: CatalogueNoticeListener,
}

fn listed() -> Listed {
    let store = Arc::new(InMemoryEventStore::new());
    let clock = Arc::new(FixedClock::new(NOTED_AT));
    let notifications = NotificationsCore::new(
        store.clone(),
        Arc::new(InMemoryNoticeCatalog::new()),
        Arc::new(InMemoryRegistry::new()),
        clock.clone(),
    );

    Listed {
        listener: CatalogueNoticeListener::new(notifications.clone()),
        notifications,
        clock,
        store,
    }
}

fn bo() -> Agent {
    Agent::Account(AccountId::from("account_bo"))
}

fn made_an_author() -> About {
    About::RoleChanged {
        project: ProjectLink::from("project_1"),
        role: Role::Author,
        by: Some(AccountId::from("account_ada")),
    }
}

async fn heard_through(listed: &Listed, notice: &NoticeId, version: u64) {
    let recorded = listed
        .store
        .read_from(&AggregateId::from(notice), KIND, Version::of(version))
        .await
        .expect("reading should succeed")
        .into_iter()
        .next()
        .expect("the change was recorded");
    let message = notifications_messaging::message_for(&recorded).expect("the change is published");

    listed
        .listener
        .take_in(&message)
        .await
        .expect("cataloguing should succeed");
}

#[tokio::test]
async fn a_noted_notice_is_listed_for_its_account_alone_until_it_is_dismissed() {
    let listed = listed();
    let notice = listed
        .notifications
        .note(
            "members:project_1@4",
            AccountId::from("account_bo"),
            made_an_author(),
        )
        .await
        .expect("noting should succeed");
    listed.clock.set(NOTED_AT + Duration::hours(1));

    heard_through(&listed, &notice, 1).await;
    let open = listed
        .notifications
        .open_for(&bo())
        .await
        .expect("listing should succeed");
    for agent in [
        Agent::Account(AccountId::from("account_ada")),
        Agent::System,
        Agent::Local,
    ] {
        assert!(
            listed
                .notifications
                .open_for(&agent)
                .await
                .expect("listing should succeed")
                .is_empty()
        );
    }
    listed
        .notifications
        .dismiss(&notice.to_string(), &bo())
        .await
        .expect("dismissing should succeed");
    heard_through(&listed, &notice, 2).await;

    assert_eq!(open.len(), 1);
    assert_eq!(open[0].id, notice);
    assert_eq!(open[0].about, made_an_author());
    assert_eq!(
        open[0].noted_at, NOTED_AT,
        "when it was noted, not when it was listed"
    );
    assert!(
        listed
            .notifications
            .open_for(&bo())
            .await
            .expect("listing should succeed")
            .is_empty(),
        "dismissed, it is listed no more"
    );
}

#[test]
fn the_list_hears_every_change_to_a_notice() {
    let listener = listed().listener;

    for key in [NOTED, DISMISSED] {
        assert!(listener.hears(&RoutingKey::parse(key).expect("a declared key")));
    }
}
