use std::sync::Arc;

use axum::Extension;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, InMemoryEventStore};
use live::Broadcaster;
use live::testing::Subscriber;
use notifications_catalog::InMemoryNoticeCatalog;
use notifications_core::{About, NoticeId, NotificationsCore, ProjectLink, Role};
use notifications_rest::{NoticeChanges, NotificationsTopic, TellingNotices};
use registry::InMemoryRegistry;
use time::OffsetDateTime;

struct Told {
    notifications: NotificationsCore,
    broadcaster: Broadcaster<NoticeChanges>,
}

fn told() -> Told {
    let broadcaster = Broadcaster::new();

    Told {
        notifications: NotificationsCore::new(
            Arc::new(InMemoryEventStore::new()),
            Arc::new(TellingNotices::around(
                Arc::new(InMemoryNoticeCatalog::new()),
                broadcaster.clone(),
            )),
            Arc::new(InMemoryRegistry::new()),
            Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
        ),
        broadcaster,
    }
}

impl Told {
    async fn following(&self, agent: Agent, about: &[(&str, &str)]) -> (TestServer, Subscriber) {
        let server = TestServer::builder().http_transport().build(
            live::live_socket(live::Topics::of(vec![Arc::new(NotificationsTopic::new(
                self.broadcaster.clone(),
            ))]))
            .layer(Extension(agent)),
        );
        let mut socket = Subscriber::on(&server).await;
        socket.subscribe("bell", "notifications", about).await;

        (server, socket)
    }

    async fn noted_and_listed(&self, account: &str, cause: &str, project: &str) -> NoticeId {
        let notice = self
            .notifications
            .note(
                cause,
                AccountId::from(account),
                About::RoleChanged {
                    project: ProjectLink::from(project),
                    role: Role::Author,
                    by: None,
                },
            )
            .await
            .expect("noting should succeed")
            .expect("its project is not deleted");
        self.notifications
            .catalogue(&notice)
            .await
            .expect("cataloguing should succeed");

        notice
    }
}

fn account(id: &str) -> Agent {
    Agent::Account(AccountId::from(id))
}

const MINE: &[(&str, &str)] = &[("mine", "true")];

#[tokio::test]
async fn an_account_is_told_each_change_to_its_own_notices_once_they_are_listed() {
    let told = told();
    let (_bo_s, mut bo) = told.following(account("account_bo"), MINE).await;
    let (_cy_s, mut cy) = told.following(account("account_cy"), MINE).await;

    let notice = told
        .noted_and_listed("account_bo", "members:project_1@4", "project_1")
        .await;
    let noted = bo.frame().await;
    told.notifications
        .dismiss(&notice.to_string(), &account("account_bo"))
        .await
        .expect("dismissing should succeed");
    told.notifications
        .catalogue(&notice)
        .await
        .expect("cataloguing should succeed");
    let dismissed = bo.frame().await;

    assert_eq!(noted["of"], notice.to_string());
    assert_eq!(dismissed["of"], notice.to_string());
    cy.nothing().await;
}

#[tokio::test]
async fn a_deleted_project_tells_whoever_had_an_open_notice_about_it() {
    let told = told();
    told.noted_and_listed("account_bo", "members:project_1@4", "project_1")
        .await;
    told.noted_and_listed("account_cy", "members:project_2@4", "project_2")
        .await;
    let (_bo_s, mut bo) = told.following(account("account_bo"), MINE).await;
    let (_cy_s, mut cy) = told.following(account("account_cy"), MINE).await;

    told.notifications
        .lay_project_tombstone(&ProjectLink::from("project_1"), OffsetDateTime::UNIX_EPOCH)
        .await
        .expect("laying a tombstone should succeed");

    assert_eq!(
        bo.frame().await["of"],
        "project_1",
        "Bo's notice about it is listed no more, so Bo's bell asks again"
    );
    cy.nothing().await;
}

#[tokio::test]
async fn only_an_account_follows_and_only_its_own() {
    let told = told();

    for (agent, about) in [
        (account("account_bo"), &[][..]),
        (Agent::Local, MINE),
        (Agent::System, MINE),
    ] {
        let (_server, mut socket) = told.following(agent, about).await;

        assert_eq!(socket.ended().await, "not-there");
    }
}
