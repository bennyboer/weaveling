use std::collections::HashMap;
use std::sync::{Arc, Mutex};

use acting::Acting;
use async_trait::async_trait;
use authorizing::{Act, Authorizer, AuthorizerError, Readable};
use axum::Extension;
use axum::http::StatusCode;
use axum::http::header::ORIGIN;
use axum_test::{TestResponse, TestServer};
use eventsourcing::{AccountId, Agent, AggregateId, EventStore, Version};
use invitations_contract::{
    InvitationPageDTO, InvitationSummaryDTO, InviteRequest, ScopeDTO, refused, role,
};
use invitations_core::{InvitationEvent, InvitationId, KIND, ProjectLink, Role, Scope};
use serde_json::Value;

use crate::wiring;
use crate::wiring::{ORIGIN as INSTANCE, Wired};

const PROJECT: &str = "project_loom";
const OWNER: [Act; 3] = [Act::Read, Act::Write, Act::Manage];
const AUTHOR: [Act; 2] = [Act::Read, Act::Write];
const BO: &str = "bo@example.com";

#[derive(Default)]
struct Roles {
    held: Mutex<HashMap<String, Vec<Act>>>,
    down: Mutex<bool>,
}

#[async_trait]
impl Authorizer for Roles {
    async fn may(&self, agent: &Agent, act: Act, project: &str) -> Result<bool, AuthorizerError> {
        if *self.down.lock().expect("never poisoned") {
            return Err(AuthorizerError::Unreachable("the list is down".into()));
        }
        let Agent::Account(account) = agent else {
            return Ok(false);
        };

        Ok(project == PROJECT
            && self
                .held
                .lock()
                .expect("never poisoned")
                .get(account.as_str())
                .is_some_and(|acts| acts.contains(&act)))
    }

    async fn readable_by(&self, _agent: &Agent) -> Result<Readable, AuthorizerError> {
        Ok(Readable::Everything)
    }
}

struct Guarded {
    wired: Wired,
    roles: Arc<Roles>,
}

fn guarded() -> Guarded {
    let roles = Arc::new(Roles::default());
    roles.held.lock().expect("never poisoned").extend([
        ("account_ada".to_owned(), OWNER.to_vec()),
        ("account_grace".to_owned(), AUTHOR.to_vec()),
    ]);

    Guarded {
        wired: wiring::a_workbench_guarded_by(roles.clone()),
        roles,
    }
}

fn acting(account: &str, operator: bool) -> Acting {
    Acting {
        agent: Agent::Account(account.into()),
        operator,
    }
}

fn ada() -> Acting {
    acting("account_ada", false)
}

fn served_to(guarded: &Guarded, acting: Acting) -> TestServer {
    TestServer::new(
        invitations_rest::router(guarded.wired.invitations.clone()).layer(Extension(acting)),
    )
}

async fn inviting(server: &TestServer, as_whom: &str) -> TestResponse {
    server
        .post("/invitations")
        .add_header(ORIGIN, INSTANCE)
        .json(&InviteRequest {
            email: BO.to_owned(),
            scope: ScopeDTO::Project {
                project: PROJECT.to_owned(),
                role: as_whom.to_owned(),
            },
        })
        .await
}

fn refused_as(response: &TestResponse, status: StatusCode, kind: &str) {
    response.assert_status(status);
    assert_eq!(response.json::<Value>()["type"], kind);
}

async fn events_of(wired: &Wired, id: &InvitationId) -> Vec<InvitationEvent> {
    wired
        .store
        .read_from(&AggregateId::from(id), KIND, Version::of(1))
        .await
        .expect("reading should succeed")
        .into_iter()
        .map(|recorded| recorded.event)
        .collect()
}

#[tokio::test]
async fn an_owner_invites_into_the_project_for_a_role_and_the_link_is_mailed() {
    // given: a project ada owns
    let guarded = guarded();

    // when: she invites into it for a role
    let response = inviting(&served_to(&guarded, ada()), role::AUTHOR).await;

    // then: the invitation is told as into the project for that role
    response.assert_status(StatusCode::CREATED);
    let summary: InvitationSummaryDTO = response.json();
    assert_eq!(
        summary.scope,
        ScopeDTO::Project {
            project: PROJECT.to_owned(),
            role: role::AUTHOR.to_owned()
        }
    );
    assert_eq!(summary.status, "open");
    // and: it is issued so by no operator
    let id: InvitationId = summary.id.parse().expect("an invitation's id");
    assert!(matches!(
        events_of(&guarded.wired, &id).await.as_slice(),
        [InvitationEvent::Issued { scope, by_an_operator: false, .. }]
            if *scope == Scope::Project { project: ProjectLink::from(PROJECT), role: Role::Author }
    ));
    // and: one mail says as what
    let mailed = guarded.wired.sent_to(BO).await;
    assert_eq!(mailed.len(), 1);
    assert!(
        mailed[0].body.contains("as an author"),
        "the mail says as what: {}",
        mailed[0].body
    );
    // and: it carries the link
    assert!(
        guarded.wired.links_mailed_to(BO).await[0]
            .starts_with("https://weaveling.example.com/invitation#")
    );
}

#[tokio::test]
async fn an_invitation_issued_is_answered_with_who_issued_it() {
    // given: a project ada owns
    let guarded = guarded();

    // when: she invites into it
    let issued = guarded
        .wired
        .invitations
        .invite_to_project(&ada(), PROJECT, Role::Author, BO, INSTANCE)
        .await
        .expect("an owner may invite");

    // then: she is answered as who issued it
    assert_eq!(issued.issued_by, Some(AccountId::from("account_ada")));
}

#[tokio::test]
async fn an_owner_who_runs_the_instance_issues_an_invitation_that_may_bring_in_a_new_address() {
    // given: a project ada owns
    let guarded = guarded();

    // when: she invites into it while running the instance
    let response = inviting(
        &served_to(&guarded, acting("account_ada", true)),
        role::READER,
    )
    .await;

    // then: the invitation is issued as by an operator
    response.assert_status(StatusCode::CREATED);
    let id: InvitationId = response
        .json::<InvitationSummaryDTO>()
        .id
        .parse()
        .expect("an invitation's id");
    assert!(matches!(
        events_of(&guarded.wired, &id).await.as_slice(),
        [InvitationEvent::Issued {
            by_an_operator: true,
            ..
        }]
    ));
}

#[tokio::test]
async fn only_an_owner_invites_and_whoever_may_not_read_the_project_finds_it_not_there() {
    // given: a project ada owns and grace authors
    let guarded = guarded();

    // then: grace inviting into it is refused as no owner
    refused_as(
        &inviting(
            &served_to(&guarded, acting("account_grace", false)),
            role::READER,
        )
        .await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OWNER,
    );
    // and: whoever may not read it finds it not there
    for stranger in [acting("account_cy", false), acting("account_op", true)] {
        refused_as(
            &inviting(&served_to(&guarded, stranger), role::READER).await,
            StatusCode::NOT_FOUND,
            "not-found",
        );
    }
    // and: nothing is mailed
    assert!(guarded.wired.sent_to(BO).await.is_empty());
}

#[tokio::test]
async fn an_invitation_into_a_project_names_a_role_there_is() {
    // given: a project ada owns
    let guarded = guarded();
    let server = served_to(&guarded, ada());

    // then: inviting for a role there is not is refused
    refused_as(
        &inviting(&server, "editor").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::ROLE_UNKNOWN,
    );
}

#[tokio::test]
async fn whom_to_let_invite_not_being_known_is_a_failure_not_a_no() {
    // given: a project whose roles cannot be told
    let guarded = guarded();
    *guarded.roles.down.lock().expect("never poisoned") = true;

    // then: inviting into it is a failure
    inviting(&served_to(&guarded, ada()), role::READER)
        .await
        .assert_status(StatusCode::INTERNAL_SERVER_ERROR);
}

async fn issued_and_listed(guarded: &Guarded) -> (InvitationId, InvitationId) {
    let into_the_project = guarded
        .wired
        .invitations
        .invite_to_project(&ada(), PROJECT, Role::Reader, BO, INSTANCE)
        .await
        .expect("an owner may invite")
        .id;
    let to_the_instance = guarded
        .wired
        .invitations
        .invite_to_instance(&wiring::an_operator(), "cy@example.com", INSTANCE)
        .await
        .expect("an operator may invite")
        .id;
    for id in [into_the_project, to_the_instance] {
        guarded
            .wired
            .invitations
            .catalogue(None, &id)
            .await
            .expect("cataloguing should succeed");
    }

    (into_the_project, to_the_instance)
}

async fn listed_by(guarded: &Guarded, acting: Acting, path: &str) -> TestResponse {
    served_to(guarded, acting).get(path).await
}

#[tokio::test]
async fn an_owner_lists_the_project_s_invitations_and_the_operators_list_keeps_to_the_instance() {
    // given: a listed invitation into a project and one to the instance
    let guarded = guarded();
    let (into_the_project, to_the_instance) = issued_and_listed(&guarded).await;

    // when: the project's owner lists the project's invitations
    let theirs = listed_by(&guarded, ada(), &format!("/invitations?project={PROJECT}")).await;
    // and: an operator lists the instance's
    let operators = listed_by(&guarded, wiring::an_operator(), "/invitations").await;

    // then: the owner is told the project's invitation alone, with its role
    theirs.assert_status_ok();
    let theirs = theirs.json::<InvitationPageDTO>().invitations;
    assert_eq!(
        theirs
            .iter()
            .map(|listed| listed.id.clone())
            .collect::<Vec<_>>(),
        vec![into_the_project.to_string()]
    );
    assert_eq!(
        theirs[0].scope,
        ScopeDTO::Project {
            project: PROJECT.to_owned(),
            role: role::READER.to_owned()
        }
    );
    // and: the operator is told the instance's alone
    assert_eq!(
        operators
            .json::<InvitationPageDTO>()
            .invitations
            .iter()
            .map(|listed| listed.id.clone())
            .collect::<Vec<_>>(),
        vec![to_the_instance.to_string()]
    );
}

#[tokio::test]
async fn only_an_owner_lists_a_project_s_invitations() {
    // given: a listed invitation into a project ada owns and grace authors
    let guarded = guarded();
    issued_and_listed(&guarded).await;
    let at = format!("/invitations?project={PROJECT}");

    // then: grace listing the project's invitations is refused as no owner
    refused_as(
        &listed_by(&guarded, acting("account_grace", false), &at).await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OWNER,
    );
    // and: an operator who may not read the project finds it not there
    refused_as(
        &listed_by(&guarded, acting("account_op", true), &at).await,
        StatusCode::NOT_FOUND,
        "not-found",
    );
}

#[tokio::test]
async fn an_owner_withdraws_a_project_s_invitation_which_nobody_else_may() {
    // given: a listed invitation into a project ada owns and grace authors
    let guarded = guarded();
    let (into_the_project, _) = issued_and_listed(&guarded).await;
    let at = format!("/invitations/{into_the_project}");

    // then: grace withdrawing it is refused as no owner
    refused_as(
        &served_to(&guarded, acting("account_grace", false))
            .delete(&at)
            .await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OWNER,
    );
    // and: whoever may not read the project finds it not there
    for stranger in [acting("account_cy", false), acting("account_op", true)] {
        refused_as(
            &served_to(&guarded, stranger).delete(&at).await,
            StatusCode::NOT_FOUND,
            refused::INVITATION_NOT_FOUND,
        );
    }
    // when: ada withdraws it
    served_to(&guarded, ada())
        .delete(&at)
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: it is withdrawn
    assert!(matches!(
        events_of(&guarded.wired, &into_the_project)
            .await
            .as_slice(),
        [InvitationEvent::Issued { .. }, InvitationEvent::Withdrawn]
    ));
}
