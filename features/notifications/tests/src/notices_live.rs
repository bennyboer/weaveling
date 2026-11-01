use std::sync::Arc;

use axum::Extension;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, InMemoryEventStore, Version};
use live::Broadcaster;
use live::testing::Subscriber;
use notifications_catalog::InMemoryNoticeCatalog;
use notifications_core::{
    About, NoticeCatalog, NoticeId, NoticeSummary, NotificationsCore, ProjectLink, Role,
};
use notifications_rest::{NoticeChanges, NotificationsTopic, TellingNotices};
use time::OffsetDateTime;
use transacting::{InMemoryTransactions, Transactions};

struct Told {
    notifications: NotificationsCore,
    broadcaster: Broadcaster<NoticeChanges>,
}

fn told() -> Told {
    let broadcaster = Broadcaster::new();

    Told {
        notifications: NotificationsCore::new(
            Arc::new(InMemoryEventStore::new()),
            Arc::new(InMemoryTransactions::new()),
            Arc::new(TellingNotices::around(
                Arc::new(InMemoryNoticeCatalog::new()),
                broadcaster.clone(),
            )),
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

    async fn noted_and_listed(&self, account: &str, project: &str) -> NoticeId {
        let notice = self
            .notifications
            .note(
                None,
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

    let notice = told.noted_and_listed("account_bo", "project_1").await;
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
    told.noted_and_listed("account_bo", "project_1").await;
    told.noted_and_listed("account_cy", "project_2").await;
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

fn bo_s_notice() -> NoticeSummary {
    NoticeSummary {
        id: NoticeId::generate(OffsetDateTime::UNIX_EPOCH),
        version: Version::of(1),
        account: AccountId::from("account_bo"),
        about: About::RoleChanged {
            project: ProjectLink::from("project_1"),
            role: Role::Author,
            by: None,
        },
        noted_at: OffsetDateTime::UNIX_EPOCH,
        dismissed_at: None,
    }
}

#[tokio::test]
async fn nobody_is_told_of_a_notice_remembered_in_a_transaction_that_is_dropped() {
    let told = told();
    let telling = TellingNotices::around(
        Arc::new(InMemoryNoticeCatalog::new()),
        told.broadcaster.clone(),
    );
    let (_bo_s, mut bo) = told.following(account("account_bo"), MINE).await;
    let transactions = InMemoryTransactions::new();
    let mut transaction = transactions
        .begin()
        .await
        .expect("beginning should succeed");
    telling
        .remember(&mut transaction, &bo_s_notice())
        .await
        .expect("remembering should succeed");

    drop(transaction);

    bo.nothing().await;
}

#[tokio::test]
async fn nobody_is_told_of_a_project_tombstone_laid_in_a_transaction_that_is_dropped() {
    let told = told();
    let telling = TellingNotices::around(
        Arc::new(InMemoryNoticeCatalog::new()),
        told.broadcaster.clone(),
    );
    let transactions = InMemoryTransactions::new();
    let mut kept = transactions
        .begin()
        .await
        .expect("beginning should succeed");
    telling
        .remember(&mut kept, &bo_s_notice())
        .await
        .expect("remembering should succeed");
    kept.finish().await.expect("finishing should succeed");
    let (_bo_s, mut bo) = told.following(account("account_bo"), MINE).await;
    let mut transaction = transactions
        .begin()
        .await
        .expect("beginning should succeed");
    telling
        .lay_project_tombstone(
            &mut transaction,
            &ProjectLink::from("project_1"),
            OffsetDateTime::UNIX_EPOCH,
        )
        .await
        .expect("laying a tombstone should succeed");

    drop(transaction);

    bo.nothing().await;
}
