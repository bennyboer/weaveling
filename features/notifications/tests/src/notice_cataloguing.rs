use std::sync::Arc;

use clock::FixedClock;
use eventsourcing::{AccountId, Agent, AggregateId, EventStore, InMemoryEventStore, Version};
use inbox::{InMemoryInbox, Inboxed};
use messaging::{Listener, RoutingKey};
use notifications_catalog::InMemoryNoticeCatalog;
use notifications_contract::{DISMISSED, NOTED};
use notifications_core::{
    About, KIND, NoticeEvent, NoticeId, NotificationsCore, ProjectLink, Role,
};
use notifications_messaging::CatalogueNoticeListener;
use time::{Duration, OffsetDateTime};
use transacting::InMemoryTransactions;

const NOTED_AT: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

struct Listed {
    store: Arc<InMemoryEventStore<NoticeEvent>>,
    clock: Arc<FixedClock>,
    notifications: NotificationsCore,
    listener: Inboxed<CatalogueNoticeListener>,
}

fn a_workbench() -> Listed {
    let store = Arc::new(InMemoryEventStore::new());
    let clock = Arc::new(FixedClock::new(NOTED_AT));
    let notifications = NotificationsCore::new(
        store.clone(),
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryNoticeCatalog::new()),
        clock.clone(),
    );

    Listed {
        listener: Inboxed::new(
            CatalogueNoticeListener::new(notifications.clone()),
            Arc::new(InMemoryTransactions::new()),
            Arc::new(InMemoryInbox::new(clock.clone())),
        ),
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

async fn hearing(listed: &Listed, notice: &NoticeId, version: u64) {
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
        .handle(&message)
        .await
        .expect("cataloguing should succeed");
}

#[tokio::test]
async fn a_noted_notice_is_listed_for_its_account_alone_until_it_is_dismissed() {
    // given: a notice noted for bo
    let listed = a_workbench();
    let notice = listed
        .notifications
        .note(None, AccountId::from("account_bo"), made_an_author())
        .await
        .expect("noting should succeed")
        .expect("its project is not deleted");
    // and: an hour gone by
    listed.clock.set(NOTED_AT + Duration::hours(1));

    // when: its noting is heard
    hearing(&listed, &notice, 1).await;
    // and: bo's open notices are read
    let open = listed
        .notifications
        .open_for(&bo())
        .await
        .expect("listing should succeed");
    // then: nobody else's hold it
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
    // when: bo dismisses it and that is heard
    listed
        .notifications
        .dismiss(&notice.to_string(), None, &bo())
        .await
        .expect("dismissing should succeed");
    hearing(&listed, &notice, 2).await;

    // then: bo's held it, as noted
    assert_eq!(open.len(), 1);
    assert_eq!(open[0].id, notice);
    assert_eq!(open[0].about, made_an_author());
    assert_eq!(
        open[0].noted_at, NOTED_AT,
        "when it was noted, not when it was listed"
    );
    // and: it is listed no more
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
    // given: the list's listener
    let listener = a_workbench().listener;

    // then: it hears every change to a notice
    for key in [NOTED, DISMISSED] {
        assert!(listener.hears(&RoutingKey::parse(key).expect("a declared key")));
    }
}
