use std::sync::Arc;

use authorizing::AllowingEverything;
use axum::Extension;
use axum::http::StatusCode;
use axum::http::header::ETAG;
use axum_test::{TestResponse, TestServer};
use clock::FixedClock;
use eventsourcing::{
    AccountId, Agent, AggregateId, EventStore, InMemoryEventStore, ServiceError, Version,
};
use members_catalog::InMemoryMembershipCatalog;
use members_contract::{ChangeRoleRequest, MemberDTO, MembershipDTO, refused, role};
use members_core::{
    KIND, MembersAuthorizer, MembersCore, MembersCoreError, MembersError, MembersEvent, Person,
    ProjectLink, Role,
};
use serde_json::Value;
use time::OffsetDateTime;
use transacting::InMemoryTransactions;

const PROJECT: &str = "project_1";

struct Members {
    core: MembersCore,
    store: Arc<InMemoryEventStore<MembersEvent>>,
}

impl Members {
    async fn told(&self) {
        self.core
            .catalogue(None, &ProjectLink::from(PROJECT))
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
        Arc::new(InMemoryTransactions::new()),
        catalog.clone(),
        Arc::new(MembersAuthorizer::new(catalog)),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );
    core.found(None, PROJECT, "account_ada")
        .await
        .expect("founding should succeed");
    for (account, joining) in [
        ("account_grace", Role::Author),
        ("account_bo", Role::Reader),
    ] {
        core.join(None, PROJECT, account, joining)
            .await
            .expect("joining should succeed");
    }
    for account in ["account_ada", "account_grace"] {
        let (name, email) = person_of(account).expect("ada and grace signed up");
        core.remember_person(
            None,
            &Person {
                account: AccountId::from(account),
                name: name.to_owned(),
                email: email.to_owned(),
            },
        )
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

async fn changing_the_role(server: &TestServer, account: &str, to: &str) -> TestResponse {
    server
        .patch(&format!("/members/{PROJECT}/accounts/{account}"))
        .json(&ChangeRoleRequest {
            role: to.to_owned(),
        })
        .await
}

#[tokio::test]
async fn whoever_may_read_the_project_lists_its_members_and_nobody_else_finds_it() {
    // given: a project of three
    let members = a_project_of_three().await;

    // when: its reader lists its members
    let listed = members
        .as_account("account_bo")
        .get(&format!("/members?project={PROJECT}"))
        .await;

    // then: they see every member by name and role, without addresses
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
    // and: a stranger finds no such project
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
async fn one_s_own_memberships_are_listed_with_one_s_role_and_nobody_else_s() {
    // given: a project of three
    let members = a_project_of_three().await;

    // when: its author and a stranger list their own memberships
    let grace_s = members
        .as_account("account_grace")
        .get("/members/mine")
        .await;
    let cy_s = members.as_account("account_cy").get("/members/mine").await;

    // then: the author sees the project as author
    grace_s.assert_status_ok();
    assert_eq!(
        grace_s.json::<Vec<MembershipDTO>>(),
        vec![MembershipDTO {
            project: PROJECT.to_owned(),
            role: role::AUTHOR.to_owned(),
        }]
    );
    // and: the stranger sees none
    cy_s.assert_status_ok();
    assert!(cy_s.json::<Vec<MembershipDTO>>().is_empty());
}

#[tokio::test]
async fn an_owner_sees_the_members_addresses_and_whoever_never_signed_up_is_listed_unnamed() {
    // given: a project of three, one never signed up
    let members = a_project_of_three().await;

    // expect: its owner sees every member's name and address, and none for the one never signed up
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
    // given: a project of three
    let members = a_project_of_three().await;
    let ada = members.as_account("account_ada");

    // when: its owner makes the author a reader
    let changed = changing_the_role(&ada, "account_grace", role::READER).await;
    // then: the owner is answered with the members as they are
    changed.assert_status_ok();
    assert_eq!(
        changed.json::<Vec<MemberDTO>>()[1],
        a_member("account_grace", role::READER)
    );

    // when: the owner makes the reader an author and the other an owner
    changing_the_role(&ada, "account_bo", role::AUTHOR)
        .await
        .assert_status_ok();
    changing_the_role(&ada, "account_grace", role::OWNER)
        .await
        .assert_status_ok();
    // then: the members hold those roles
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
    // given: a project of three
    let members = a_project_of_three().await;

    // expect: its author may not change a role
    refused_as(
        changing_the_role(
            &members.as_account("account_grace"),
            "account_bo",
            role::AUTHOR,
        )
        .await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OWNER,
    );
    // and: a stranger finds no such project
    refused_as(
        changing_the_role(
            &members.as_account("account_cy"),
            "account_bo",
            role::AUTHOR,
        )
        .await,
        StatusCode::NOT_FOUND,
        "not-found",
    );
    // and: the author may not remove anyone
    refused_as(
        members
            .as_account("account_grace")
            .delete(&format!("/members/{PROJECT}/accounts/account_bo"))
            .await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OWNER,
    );
    // and: the reader stays a reader
    assert_eq!(
        members.listed().await[2],
        a_member("account_bo", role::READER)
    );
}

#[tokio::test]
async fn the_role_of_someone_who_is_not_a_member_is_not_changed() {
    // given: a project of three
    let members = a_project_of_three().await;

    // expect: its owner changing a stranger's role is refused as no member
    refused_as(
        changing_the_role(
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
    // given: a project of three
    let members = a_project_of_three().await;

    // expect: giving a role there is not is refused
    refused_as(
        changing_the_role(
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
    // given: a project of three with one owner
    let members = a_project_of_three().await;
    let ada = members.as_account("account_ada");

    // expect: the owner stepping down is refused
    refused_as(
        changing_the_role(&ada, "account_ada", role::AUTHOR).await,
        StatusCode::CONFLICT,
        refused::LAST_OWNER,
    );
    // and: the owner leaving is refused
    refused_as(
        ada.delete(&format!("/members/{PROJECT}/accounts/account_ada"))
            .await,
        StatusCode::CONFLICT,
        refused::LAST_OWNER,
    );
    // and: they stay owner
    assert_eq!(
        members.listed().await[0],
        a_member("account_ada", role::OWNER)
    );
}

#[tokio::test]
async fn an_owner_removes_a_member_and_removing_one_gone_already_changes_nothing() {
    // given: a project of three
    let members = a_project_of_three().await;
    let ada = members.as_account("account_ada");

    // when: its owner removes the reader twice
    for _ in 0..2 {
        ada.delete(&format!("/members/{PROJECT}/accounts/account_bo"))
            .await
            .assert_status(StatusCode::NO_CONTENT);
    }

    // then: the others are left
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
    // given: a project of three
    let members = a_project_of_three().await;

    // expect: its reader leaves
    members
        .as_account("account_bo")
        .delete(&format!("/members/{PROJECT}/accounts/account_bo"))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    // and: a stranger finds no such project to leave
    refused_as(
        members
            .as_account("account_cy")
            .delete(&format!("/members/{PROJECT}/accounts/account_cy"))
            .await,
        StatusCode::NOT_FOUND,
        "not-found",
    );

    // and: two members are left
    assert_eq!(members.listed().await.len(), 2);
}

#[tokio::test]
async fn leaving_asks_whether_one_is_a_member_not_whether_one_may_read() {
    // given: a project, with everyone allowed to read everything
    let store = Arc::new(InMemoryEventStore::new());
    let core = MembersCore::new(
        store,
        Arc::new(InMemoryTransactions::new()),
        Arc::new(InMemoryMembershipCatalog::new()),
        Arc::new(AllowingEverything),
        Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH)),
    );
    core.found(None, PROJECT, "account_ada")
        .await
        .expect("founding should succeed");
    // and: a stranger
    let cy = Agent::Account(AccountId::from("account_cy"));

    // expect: the stranger finds no such project to leave, nor one never started
    for project in [PROJECT, "project_never_started"] {
        assert!(
            matches!(
                core.leave(project, None, &cy).await,
                Err(MembersCoreError::ProjectNotThere)
            ),
            "{project}: whoever is not in a project finds nothing to leave, whatever they may read"
        );
    }
}

#[tokio::test]
async fn the_system_is_no_member_and_has_nothing_to_leave() {
    // given: a project of three
    let members = a_project_of_three().await;

    // expect: the system leaving it is refused as no member
    assert!(matches!(
        members.core.leave(PROJECT, None, &Agent::System).await,
        Err(MembersCoreError::Events(ServiceError::Refused(
            MembersError::NotMember
        )))
    ));
    // and: all three stay
    assert_eq!(members.listed().await.len(), 3);
}

#[tokio::test]
async fn joining_is_the_system_s_carrying_out_what_an_invitation_decided() {
    // given: a project of three, two who joined
    let members = a_project_of_three().await;

    // when: what happened to its members is read
    let recorded = members
        .store
        .read_from(
            &AggregateId::from(&ProjectLink::from(PROJECT)),
            KIND,
            Version::of(1),
        )
        .await
        .expect("reading should succeed");

    // then: every joining was done by the system
    assert!(
        recorded
            .iter()
            .filter(|recorded| matches!(recorded.event, MembersEvent::Joined { .. }))
            .all(|recorded| recorded.metadata.agent == Agent::System)
    );
}

async fn the_listed_version(members: &Members) -> String {
    let listed = members
        .as_account("account_ada")
        .get(&format!("/members?project={PROJECT}"))
        .await;
    listed.assert_status_ok();

    listed
        .header(ETAG)
        .to_str()
        .expect("a tag is text")
        .to_owned()
}

async fn changing_the_role_from(
    server: &TestServer,
    account: &str,
    to: &str,
    version: &str,
) -> TestResponse {
    server
        .patch(&format!("/members/{PROJECT}/accounts/{account}"))
        .add_header("if-match", version)
        .json(&ChangeRoleRequest {
            role: to.to_owned(),
        })
        .await
}

async fn removing_from(server: &TestServer, account: &str, version: &str) -> TestResponse {
    server
        .delete(&format!("/members/{PROJECT}/accounts/{account}"))
        .add_header("if-match", version)
        .await
}

#[tokio::test]
async fn the_members_are_listed_with_the_version_they_are_at() {
    // given: a project of three, founded and joined twice
    let members = a_project_of_three().await;

    // when: its members are listed
    let tag = the_listed_version(&members).await;

    // then: the listing tells the version they are at
    assert_eq!(tag, "\"3\"");
}

#[tokio::test]
async fn a_change_of_role_from_the_version_listed_lands_and_tells_the_next() {
    // given: a project of three, its members listed
    let members = a_project_of_three().await;
    let listed_at = the_listed_version(&members).await;

    // when: its owner makes the author a reader from the version listed
    let changed = changing_the_role_from(
        &members.as_account("account_ada"),
        "account_grace",
        role::READER,
        &listed_at,
    )
    .await;

    // then: the change lands and tells the version it landed at
    changed.assert_status_ok();
    assert_eq!(changed.header(ETAG), "\"4\"");
    assert_eq!(
        changed.json::<Vec<MemberDTO>>()[1],
        a_member("account_grace", role::READER)
    );
}

#[tokio::test]
async fn a_change_of_role_from_a_version_moved_past_is_refused_and_changes_nothing() {
    // given: a project of three, listed before another change of role landed
    let members = a_project_of_three().await;
    let ada = members.as_account("account_ada");
    let listed_at = the_listed_version(&members).await;
    changing_the_role(&ada, "account_bo", role::AUTHOR)
        .await
        .assert_status_ok();

    // when: its owner makes the author a reader from the version listed
    let changed = changing_the_role_from(&ada, "account_grace", role::READER, &listed_at).await;

    // then: the change is refused as outdated
    refused_as(changed, StatusCode::PRECONDITION_FAILED, "outdated");
    // and: the author is still an author
    assert_eq!(
        members.listed().await[1],
        a_member("account_grace", role::AUTHOR),
        "a refused change of role must leave the role alone"
    );
}

#[tokio::test]
async fn a_change_of_role_without_if_match_lands_on_whatever_the_members_are_at() {
    // given: a project of three, moved on by a change of role
    let members = a_project_of_three().await;
    let ada = members.as_account("account_ada");
    changing_the_role(&ada, "account_bo", role::AUTHOR)
        .await
        .assert_status_ok();

    // when: its owner makes the author a reader without saying from which version
    let changed = changing_the_role(&ada, "account_grace", role::READER).await;

    // then: the change lands on the version they are at
    changed.assert_status_ok();
    assert_eq!(changed.header(ETAG), "\"5\"");
}

#[tokio::test]
async fn a_removal_from_the_version_listed_lands() {
    // given: a project of three, its members listed
    let members = a_project_of_three().await;
    let listed_at = the_listed_version(&members).await;

    // when: its owner removes the reader from the version listed
    let removed = removing_from(&members.as_account("account_ada"), "account_bo", &listed_at).await;

    // then: the removal lands
    removed.assert_status(StatusCode::NO_CONTENT);
    // and: the others are left
    assert_eq!(members.listed().await.len(), 2);
}

#[tokio::test]
async fn a_removal_from_a_version_moved_past_is_refused_and_removes_nobody() {
    // given: a project of three, listed before a change of role landed
    let members = a_project_of_three().await;
    let ada = members.as_account("account_ada");
    let listed_at = the_listed_version(&members).await;
    changing_the_role(&ada, "account_grace", role::READER)
        .await
        .assert_status_ok();

    // when: its owner removes the reader from the version listed
    let removed = removing_from(&ada, "account_bo", &listed_at).await;

    // then: the removal is refused as outdated
    refused_as(removed, StatusCode::PRECONDITION_FAILED, "outdated");
    // and: all three are still members
    assert_eq!(members.listed().await.len(), 3);
}

#[tokio::test]
async fn a_removal_without_if_match_lands_on_whatever_the_members_are_at() {
    // given: a project of three, moved on by a change of role
    let members = a_project_of_three().await;
    let ada = members.as_account("account_ada");
    changing_the_role(&ada, "account_grace", role::READER)
        .await
        .assert_status_ok();

    // when: its owner removes the reader without saying from which version
    let removed = ada
        .delete(&format!("/members/{PROJECT}/accounts/account_bo"))
        .await;

    // then: the removal lands
    removed.assert_status(StatusCode::NO_CONTENT);
    assert_eq!(members.listed().await.len(), 2);
}

#[tokio::test]
async fn leaving_from_the_version_listed_lands() {
    // given: a project of three, its members listed
    let members = a_project_of_three().await;
    let listed_at = the_listed_version(&members).await;

    // when: its reader leaves from the version listed
    let left = removing_from(&members.as_account("account_bo"), "account_bo", &listed_at).await;

    // then: the leaving lands
    left.assert_status(StatusCode::NO_CONTENT);
    assert_eq!(members.listed().await.len(), 2);
}

#[tokio::test]
async fn leaving_from_a_version_moved_past_is_refused_and_keeps_the_member() {
    // given: a project of three, listed before a change of role landed
    let members = a_project_of_three().await;
    let listed_at = the_listed_version(&members).await;
    changing_the_role(
        &members.as_account("account_ada"),
        "account_grace",
        role::READER,
    )
    .await
    .assert_status_ok();

    // when: its reader leaves from the version listed
    let left = removing_from(&members.as_account("account_bo"), "account_bo", &listed_at).await;

    // then: the leaving is refused as outdated
    refused_as(left, StatusCode::PRECONDITION_FAILED, "outdated");
    // and: all three are still members
    assert_eq!(members.listed().await.len(), 3);
}

#[tokio::test]
async fn leaving_without_if_match_lands_on_whatever_the_members_are_at() {
    // given: a project of three, moved on by a change of role
    let members = a_project_of_three().await;
    changing_the_role(
        &members.as_account("account_ada"),
        "account_grace",
        role::READER,
    )
    .await
    .assert_status_ok();

    // when: its reader leaves without saying from which version
    let left = members
        .as_account("account_bo")
        .delete(&format!("/members/{PROJECT}/accounts/account_bo"))
        .await;

    // then: the leaving lands
    left.assert_status(StatusCode::NO_CONTENT);
    assert_eq!(members.listed().await.len(), 2);
}
