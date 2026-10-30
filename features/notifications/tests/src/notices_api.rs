use std::collections::BTreeMap;
use std::sync::Arc;

use axum::Extension;
use axum::http::StatusCode;
use axum_test::TestServer;
use clock::FixedClock;
use eventsourcing::{AccountId, Agent, InMemoryEventStore, Version};
use notifications_catalog::InMemoryNoticeCatalog;
use notifications_contract::{AboutDTO, NoticeDTO};
use notifications_core::{
    About, NoticeId, NotificationsCore, Person, ProjectLink, ProjectName, Role,
};
use registry::InMemoryRegistry;
use serde_json::Value;
use time::OffsetDateTime;

struct Served {
    notifications: NotificationsCore,
}

impl Served {
    fn as_agent(&self, agent: Agent) -> TestServer {
        TestServer::new(
            notifications_rest::router(self.notifications.clone()).layer(Extension(agent)),
        )
    }

    fn as_account(&self, account: &str) -> TestServer {
        self.as_agent(Agent::Account(AccountId::from(account)))
    }

    async fn noted_for_bo(&self, cause: &str, role: Role, by: Option<&str>) -> NoticeId {
        let notice = self
            .notifications
            .note(
                cause,
                AccountId::from("account_bo"),
                About::RoleChanged {
                    project: ProjectLink::from("project_1"),
                    role,
                    by: by.map(AccountId::from),
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
}

fn served() -> Served {
    Served {
        notifications: NotificationsCore::new(
            Arc::new(InMemoryEventStore::new()),
            Arc::new(InMemoryNoticeCatalog::new()),
            Arc::new(InMemoryRegistry::new()),
            Arc::new(FixedClock::new(
                OffsetDateTime::from_unix_timestamp(1_700_000_000).expect("a plausible moment"),
            )),
        ),
    }
}

async fn listed(server: &TestServer) -> Vec<NoticeDTO> {
    let answered = server.get("/notifications").await;
    answered.assert_status_ok();

    answered.json()
}

#[tokio::test]
async fn one_s_own_open_notices_are_listed_with_what_they_are_about() {
    let served = served();
    let notice = served
        .noted_for_bo("members:project_1@4", Role::Author, Some("account_ada"))
        .await;
    served
        .notifications
        .remember_person(&Person {
            account: AccountId::from("account_ada"),
            name: "Ada Lovelace".to_owned(),
        })
        .await
        .expect("remembering should succeed");
    served
        .notifications
        .remember_project_name(&ProjectName {
            project: ProjectLink::from("project_1"),
            version: Version::of(1),
            name: "Shared".to_owned(),
        })
        .await
        .expect("remembering should succeed");

    assert_eq!(
        listed(&served.as_account("account_bo")).await,
        vec![NoticeDTO {
            id: notice.to_string(),
            noted_at: "2023-11-14T22:13:20Z".to_owned(),
            about: AboutDTO::RoleChanged {
                project: "project_1".to_owned(),
                role: "author".to_owned(),
                by: Some("account_ada".to_owned()),
            },
            names: BTreeMap::from([
                ("account_ada".to_owned(), "Ada Lovelace".to_owned()),
                ("project_1".to_owned(), "Shared".to_owned()),
            ]),
        }]
    );
    for nobody in [
        served.as_account("account_cy"),
        served.as_agent(Agent::Local),
    ] {
        assert!(listed(&nobody).await.is_empty());
    }
}

#[tokio::test]
async fn one_s_own_notice_is_dismissed_and_listed_no_more() {
    let served = served();
    let notice = served
        .noted_for_bo("members:project_1@4", Role::Reader, None)
        .await;
    let bo_s = served.as_account("account_bo");

    bo_s.post(&format!("/notifications/{notice}/dismissed"))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    bo_s.post(&format!("/notifications/{notice}/dismissed"))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    served.catalogued(&notice).await;

    assert!(listed(&bo_s).await.is_empty());
}

#[tokio::test]
async fn another_s_notice_or_none_at_all_is_not_there_to_dismiss() {
    let served = served();
    let notice = served
        .noted_for_bo("members:project_1@4", Role::Owner, Some("account_ada"))
        .await;

    for (server, path) in [
        (
            served.as_account("account_cy"),
            format!("/notifications/{notice}/dismissed"),
        ),
        (
            served.as_account("account_bo"),
            format!(
                "/notifications/{}/dismissed",
                NoticeId::generate(OffsetDateTime::UNIX_EPOCH)
            ),
        ),
        (
            served.as_account("account_bo"),
            "/notifications/no-notice/dismissed".to_owned(),
        ),
    ] {
        let answered = server.post(&path).await;
        answered.assert_status(StatusCode::NOT_FOUND);
        assert_eq!(answered.json::<Value>()["type"], "not-found");
    }
    let still = listed(&served.as_account("account_bo")).await;
    assert_eq!(
        still[0].about,
        AboutDTO::RoleChanged {
            project: "project_1".to_owned(),
            role: "owner".to_owned(),
            by: Some("account_ada".to_owned()),
        },
        "still there, and still Bo's"
    );
    assert!(
        still[0].names.is_empty(),
        "nobody and no project known by name is named"
    );
}
