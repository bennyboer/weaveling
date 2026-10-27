use std::sync::Arc;

use authorizing::AllowingEverything;
use axum::Extension;
use axum::http::StatusCode;
use axum_test::{TestResponse, TestServer};
use clock::FixedClock;
use eventsourcing::{
    AccountId, Agent, AggregateId, EventStore, InMemoryEventStore, ServiceError, Version,
};
use members_catalog::InMemoryMembershipCatalog;
use members_contract::{ChangeRoleRequest, MemberDTO, refused, role};
use members_core::{
    KIND, MembersAuthorizer, MembersCore, MembersCoreError, MembersError, MembersEvent, Person,
    ProjectLink, Role,
};
use serde_json::Value;
use time::OffsetDateTime;

const PROJECT: &str = "project_1";

struct Members {
    core: MembersCore,
    store: Arc<InMemoryEventStore<MembersEvent>>,
}

impl Members {
    async fn told(&self) {
        self.core
            .catalogue(&ProjectLink::from(PROJECT))
            .await
            .expect("cataloguing should succeed");
    }

    fn as_account(&self, account: &str) -> TestServer {
        TestServer::new(
            members_rest::router(self.core.clone())
                .layer(Extension(Agent::Account(AccountId::from(account)))),
        )
    }

    async fn listed(&self) -> Vec<MemberDTO> {
        let listed = self
            .as_account("account_ada")
            .get(&format!("/members?project={PROJECT}"))
            .await;
        listed.assert_status_ok();

        listed.json()
    }
}

async fn a_project_of_three() -> Members {
    let store = Arc::new(InMemoryEventStore::new());
    let catalog = Arc::new(InMemoryMembershipCatalog::new());
    let core = MembersCore::new(
        store.clone(),
        catalog.clone(),
        Arc::new(MembersAuthorizer::new(catalog)),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );
    core.found(PROJECT, "account_ada")
        .await
        .expect("founding should succeed");
    for (account, joining) in [
        ("account_grace", Role::Author),
        ("account_bo", Role::Reader),
    ] {
        core.join(PROJECT, account, joining)
            .await
            .expect("joining should succeed");
    }
    for account in ["account_ada", "account_grace"] {
        let (name, email) = person_of(account).expect("ada and grace signed up");
        core.remember_person(&Person {
            account: AccountId::from(account),
            name: name.to_owned(),
            email: email.to_owned(),
        })
        .await
        .expect("remembering should succeed");
    }
    let members = Members { core, store };
    members.told().await;

    members
}

fn person_of(account: &str) -> Option<(&'static str, &'static str)> {
    match account {
        "account_ada" => Some(("Ada Lovelace", "ada@example.com")),
        "account_grace" => Some(("Grace Hopper", "grace@example.com")),
        _ => None,
    }
}

fn a_member(account: &str, named: &str) -> MemberDTO {
    MemberDTO {
        account: account.to_owned(),
        role: named.to_owned(),
        name: person_of(account).map(|(name, _)| name.to_owned()),
        email: person_of(account).map(|(_, email)| email.to_owned()),
    }
}

fn as_a_member_sees(member: MemberDTO) -> MemberDTO {
    MemberDTO {
        email: None,
        ..member
    }
}

fn refused_as(response: TestResponse, status: StatusCode, kind: &str) {
    response.assert_status(status);
    assert_eq!(response.json::<Value>()["type"], kind);
}

async fn role_changed(server: &TestServer, account: &str, to: &str) -> TestResponse {
    server
        .patch(&format!("/members/{PROJECT}/accounts/{account}"))
        .json(&ChangeRoleRequest {
            role: to.to_owned(),
        })
        .await
}

#[tokio::test]
async fn whoever_may_read_the_project_lists_its_members_and_nobody_else_finds_it() {
    let members = a_project_of_three().await;

    let listed = members
        .as_account("account_bo")
        .get(&format!("/members?project={PROJECT}"))
        .await;

    listed.assert_status_ok();
    assert_eq!(
        listed.json::<Vec<MemberDTO>>(),
        vec![
            as_a_member_sees(a_member("account_ada", role::OWNER)),
            as_a_member_sees(a_member("account_grace", role::AUTHOR)),
            as_a_member_sees(a_member("account_bo", role::READER)),
        ],
        "a reader sees who is in it by name, but learns no address"
    );
    refused_as(
        members
            .as_account("account_cy")
            .get(&format!("/members?project={PROJECT}"))
            .await,
        StatusCode::NOT_FOUND,
        "not-found",
    );
}

#[tokio::test]
async fn an_owner_sees_the_members_addresses_and_whoever_never_signed_up_is_listed_unnamed() {
    let members = a_project_of_three().await;

    assert_eq!(
        members.listed().await,
        vec![
            MemberDTO {
                account: "account_ada".to_owned(),
                role: role::OWNER.to_owned(),
                name: Some("Ada Lovelace".to_owned()),
                email: Some("ada@example.com".to_owned()),
            },
            MemberDTO {
                account: "account_grace".to_owned(),
                role: role::AUTHOR.to_owned(),
                name: Some("Grace Hopper".to_owned()),
                email: Some("grace@example.com".to_owned()),
            },
            MemberDTO {
                account: "account_bo".to_owned(),
                role: role::READER.to_owned(),
                name: None,
                email: None,
            },
        ],
        "owners invite and manage, so they see whom they deal with"
    );
}

#[tokio::test]
async fn an_owner_changes_a_member_s_role_and_is_answered_with_the_members_as_they_are() {
    let members = a_project_of_three().await;
    let ada = members.as_account("account_ada");

    let changed = role_changed(&ada, "account_grace", role::READER).await;
    changed.assert_status_ok();
    assert_eq!(
        changed.json::<Vec<MemberDTO>>()[1],
        a_member("account_grace", role::READER)
    );

    role_changed(&ada, "account_bo", role::AUTHOR)
        .await
        .assert_status_ok();
    role_changed(&ada, "account_grace", role::OWNER)
        .await
        .assert_status_ok();
    assert_eq!(
        members.listed().await,
        vec![
            a_member("account_ada", role::OWNER),
            a_member("account_grace", role::OWNER),
            a_member("account_bo", role::AUTHOR),
        ],
        "every role there is can be given"
    );
}

#[tokio::test]
async fn only_an_owner_changes_roles_and_a_stranger_finds_nothing_to_change() {
    let members = a_project_of_three().await;

    refused_as(
        role_changed(
            &members.as_account("account_grace"),
            "account_bo",
            role::AUTHOR,
        )
        .await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OWNER,
    );
    refused_as(
        role_changed(
            &members.as_account("account_cy"),
            "account_bo",
            role::AUTHOR,
        )
        .await,
        StatusCode::NOT_FOUND,
        "not-found",
    );
    refused_as(
        members
            .as_account("account_grace")
            .delete(&format!("/members/{PROJECT}/accounts/account_bo"))
            .await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OWNER,
    );
    assert_eq!(
        members.listed().await[2],
        a_member("account_bo", role::READER)
    );
}

#[tokio::test]
async fn the_role_of_someone_who_is_not_a_member_is_not_changed() {
    let members = a_project_of_three().await;

    refused_as(
        role_changed(
            &members.as_account("account_ada"),
            "account_cy",
            role::AUTHOR,
        )
        .await,
        StatusCode::NOT_FOUND,
        refused::NOT_A_MEMBER,
    );
}

#[tokio::test]
async fn a_role_nobody_knows_is_refused() {
    let members = a_project_of_three().await;

    refused_as(
        role_changed(
            &members.as_account("account_ada"),
            "account_grace",
            "editor",
        )
        .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::ROLE_UNKNOWN,
    );
}

#[tokio::test]
async fn the_last_owner_neither_steps_down_nor_leaves() {
    let members = a_project_of_three().await;
    let ada = members.as_account("account_ada");

    refused_as(
        role_changed(&ada, "account_ada", role::AUTHOR).await,
        StatusCode::CONFLICT,
        refused::LAST_OWNER,
    );
    refused_as(
        ada.delete(&format!("/members/{PROJECT}/accounts/account_ada"))
            .await,
        StatusCode::CONFLICT,
        refused::LAST_OWNER,
    );
    assert_eq!(
        members.listed().await[0],
        a_member("account_ada", role::OWNER)
    );
}

#[tokio::test]
async fn an_owner_removes_a_member_and_removing_one_gone_already_changes_nothing() {
    let members = a_project_of_three().await;
    let ada = members.as_account("account_ada");

    for _ in 0..2 {
        ada.delete(&format!("/members/{PROJECT}/accounts/account_bo"))
            .await
            .assert_status(StatusCode::NO_CONTENT);
    }

    assert_eq!(
        members.listed().await,
        vec![
            a_member("account_ada", role::OWNER),
            a_member("account_grace", role::AUTHOR),
        ]
    );
}

#[tokio::test]
async fn a_member_leaves_of_their_own_accord_and_a_stranger_has_nothing_to_leave() {
    let members = a_project_of_three().await;

    members
        .as_account("account_bo")
        .delete(&format!("/members/{PROJECT}/accounts/account_bo"))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    refused_as(
        members
            .as_account("account_cy")
            .delete(&format!("/members/{PROJECT}/accounts/account_cy"))
            .await,
        StatusCode::NOT_FOUND,
        "not-found",
    );

    assert_eq!(members.listed().await.len(), 2);
}

#[tokio::test]
async fn leaving_asks_whether_one_is_a_member_not_whether_one_may_read() {
    let store = Arc::new(InMemoryEventStore::new());
    let core = MembersCore::new(
        store,
        Arc::new(InMemoryMembershipCatalog::new()),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );
    core.found(PROJECT, "account_ada")
        .await
        .expect("founding should succeed");
    let cy = Agent::Account(AccountId::from("account_cy"));

    for project in [PROJECT, "project_never_started"] {
        assert!(
            matches!(
                core.leave(project, &cy).await,
                Err(MembersCoreError::ProjectNotThere)
            ),
            "{project}: whoever is not in a project finds nothing to leave, whatever they may read"
        );
    }
}

#[tokio::test]
async fn the_system_is_no_member_and_has_nothing_to_leave() {
    let members = a_project_of_three().await;

    assert!(matches!(
        members.core.leave(PROJECT, &Agent::System).await,
        Err(MembersCoreError::Events(ServiceError::Refused(
            MembersError::NotMember
        )))
    ));
    assert_eq!(members.listed().await.len(), 3);
}

#[tokio::test]
async fn joining_is_the_system_s_carrying_out_what_an_invitation_decided() {
    let members = a_project_of_three().await;

    let recorded = members
        .store
        .read_from(
            &AggregateId::from(&ProjectLink::from(PROJECT)),
            KIND,
            Version::of(1),
        )
        .await
        .expect("reading should succeed");

    assert!(
        recorded
            .iter()
            .filter(|recorded| matches!(recorded.event, MembersEvent::Joined { .. }))
            .all(|recorded| recorded.metadata.agent == Agent::System)
    );
}
