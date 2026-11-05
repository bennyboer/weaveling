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
use serde_json::Value;
use time::OffsetDateTime;
use transacting::InMemoryTransactions;

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

    async fn a_listed_notice_for_bo(&self, role: Role, by: Option<&str>) -> NoticeId {
        let notice = self
            .notifications
            .note(
                None,
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
            .catalogue(None, notice)
            .await
            .expect("cataloguing should succeed");
    }

    async fn standing_of(&self, notice: &NoticeId) -> (bool, Version) {
        let standing = self
            .notifications
            .read(
                &notice.to_string(),
                &Agent::Account(AccountId::from("account_bo")),
            )
            .await
            .expect("bo's notice is there");

        (standing.state.is_dismissed(), standing.version)
    }
}

fn a_workbench() -> Served {
    Served {
        notifications: NotificationsCore::new(
            Arc::new(InMemoryEventStore::new()),
            Arc::new(InMemoryTransactions::new()),
            Arc::new(InMemoryNoticeCatalog::new()),
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
    // given: a listed notice for bo
    let served = a_workbench();
    let notice = served
        .a_listed_notice_for_bo(Role::Author, Some("account_ada"))
        .await;
    // and: who changed the role and the project known by name
    served
        .notifications
        .remember_person(
            None,
            &Person {
                account: AccountId::from("account_ada"),
                name: "Ada Lovelace".to_owned(),
            },
        )
        .await
        .expect("remembering should succeed");
    served
        .notifications
        .remember_project_name(
            None,
            &ProjectName {
                project: ProjectLink::from("project_1"),
                version: Version::of(1),
                name: "Shared".to_owned(),
            },
        )
        .await
        .expect("remembering should succeed");

    // then: bo's list holds it with what it is about and those names
    assert_eq!(
        listed(&served.as_account("account_bo")).await,
        vec![NoticeDTO {
            id: notice.to_string(),
            version: 1,
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
    // and: nobody else's holds anything
    for nobody in [
        served.as_account("account_cy"),
        served.as_agent(Agent::Local),
    ] {
        assert!(listed(&nobody).await.is_empty());
    }
}

#[tokio::test]
async fn one_s_own_notice_is_dismissed_and_listed_no_more() {
    // given: a listed notice for bo
    let served = a_workbench();
    let notice = served.a_listed_notice_for_bo(Role::Reader, None).await;
    let bo_s = served.as_account("account_bo");

    // when: bo dismisses it twice
    bo_s.post(&format!("/notifications/{notice}/dismissed"))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    bo_s.post(&format!("/notifications/{notice}/dismissed"))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    served.catalogued(&notice).await;

    // then: it is listed no more
    assert!(listed(&bo_s).await.is_empty());
}

#[tokio::test]
async fn another_s_notice_or_none_at_all_is_not_there_to_dismiss() {
    // given: a listed notice for bo
    let served = a_workbench();
    let notice = served
        .a_listed_notice_for_bo(Role::Owner, Some("account_ada"))
        .await;

    // when: another dismisses it, or bo dismisses a notice nobody noted or no notice at all
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
        // then: it is not there
        answered.assert_status(StatusCode::NOT_FOUND);
        assert_eq!(answered.json::<Value>()["type"], "not-found");
    }
    // and: bo's notice stands as it was, naming nobody it does not know
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

#[tokio::test]
async fn a_notice_dismissed_from_the_version_its_owner_saw_is_dismissed() {
    // given: a listed notice for bo at its first version
    let served = a_workbench();
    let notice = served.a_listed_notice_for_bo(Role::Reader, None).await;

    // when: bo dismisses it from that version
    served
        .as_account("account_bo")
        .post(&format!("/notifications/{notice}/dismissed"))
        .add_header("if-match", "\"1\"")
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: it is dismissed at the next version
    assert_eq!(served.standing_of(&notice).await, (true, Version::of(2)));
}

#[tokio::test]
async fn a_notice_dismissed_from_a_version_it_has_moved_past_is_refused_and_changes_nothing() {
    // given: a notice bo dismissed already, elsewhere
    let served = a_workbench();
    let notice = served.a_listed_notice_for_bo(Role::Reader, None).await;
    let bo_s = served.as_account("account_bo");
    bo_s.post(&format!("/notifications/{notice}/dismissed"))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // when: bo dismisses it from the version it saw first
    let answered = bo_s
        .post(&format!("/notifications/{notice}/dismissed"))
        .add_header("if-match", "\"1\"")
        .await;

    // then: it is refused as outdated
    answered.assert_status(StatusCode::PRECONDITION_FAILED);
    assert_eq!(answered.json::<Value>()["type"], "outdated");
    // and: the notice stays as it was
    assert_eq!(served.standing_of(&notice).await, (true, Version::of(2)));
}

#[tokio::test]
async fn a_notice_dismissed_without_a_version_is_dismissed_whatever_it_stands_at() {
    // given: a listed notice for bo
    let served = a_workbench();
    let notice = served.a_listed_notice_for_bo(Role::Reader, None).await;

    // when: bo dismisses it naming no version
    served
        .as_account("account_bo")
        .post(&format!("/notifications/{notice}/dismissed"))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: it is dismissed
    assert_eq!(served.standing_of(&notice).await, (true, Version::of(2)));
}
