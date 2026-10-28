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
        wired: wiring::wired_guarded_by(roles.clone()),
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

async fn invited(server: &TestServer, as_whom: &str) -> TestResponse {
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

async fn told(wired: &Wired, id: &InvitationId) -> Vec<InvitationEvent> {
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
    let guarded = guarded();

    let response = invited(&served_to(&guarded, ada()), role::AUTHOR).await;

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
    let id: InvitationId = summary.id.parse().expect("an invitation's id");
    assert!(matches!(
        told(&guarded.wired, &id).await.as_slice(),
        [InvitationEvent::Issued { scope, by_an_operator: false, .. }]
            if *scope == Scope::Project { project: ProjectLink::from(PROJECT), role: Role::Author }
    ));
    let mailed = guarded.wired.mailbox.sent_to(BO);
    assert_eq!(mailed.len(), 1);
    assert!(
        mailed[0].body.contains("as an author"),
        "the mail says as what: {}",
        mailed[0].body
    );
    assert!(
        guarded.wired.links_mailed_to(BO)[0]
            .starts_with("https://weaveling.example.com/invitation#")
    );
}

#[tokio::test]
async fn an_invitation_issued_is_answered_with_who_issued_it() {
    let guarded = guarded();

    let issued = guarded
        .wired
        .invitations
        .invite_to_project(&ada(), PROJECT, Role::Author, BO, INSTANCE)
        .await
        .expect("an owner may invite");

    assert_eq!(issued.issued_by, Some(AccountId::from("account_ada")));
}

#[tokio::test]
async fn an_owner_who_runs_the_instance_issues_an_invitation_that_may_bring_in_a_new_address() {
    let guarded = guarded();

    let response = invited(
        &served_to(&guarded, acting("account_ada", true)),
        role::READER,
    )
    .await;

    response.assert_status(StatusCode::CREATED);
    let id: InvitationId = response
        .json::<InvitationSummaryDTO>()
        .id
        .parse()
        .expect("an invitation's id");
    assert!(matches!(
        told(&guarded.wired, &id).await.as_slice(),
        [InvitationEvent::Issued {
            by_an_operator: true,
            ..
        }]
    ));
}

#[tokio::test]
async fn only_an_owner_invites_and_whoever_may_not_read_the_project_finds_it_not_there() {
    let guarded = guarded();

    refused_as(
        &invited(
            &served_to(&guarded, acting("account_grace", false)),
            role::READER,
        )
        .await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OWNER,
    );
    for stranger in [acting("account_cy", false), acting("account_op", true)] {
        refused_as(
            &invited(&served_to(&guarded, stranger), role::READER).await,
            StatusCode::NOT_FOUND,
            "not-found",
        );
    }
    assert!(guarded.wired.mailbox.sent_to(BO).is_empty());
}

#[tokio::test]
async fn an_invitation_into_a_project_names_a_role_there_is() {
    let guarded = guarded();
    let server = served_to(&guarded, ada());

    refused_as(
        &invited(&server, "editor").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::ROLE_UNKNOWN,
    );
}

#[tokio::test]
async fn whom_to_let_invite_not_being_known_is_a_failure_not_a_no() {
    let guarded = guarded();
    *guarded.roles.down.lock().expect("never poisoned") = true;

    invited(&served_to(&guarded, ada()), role::READER)
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
            .catalogue(&id)
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
    let guarded = guarded();
    let (into_the_project, to_the_instance) = issued_and_listed(&guarded).await;

    let theirs = listed_by(&guarded, ada(), &format!("/invitations?project={PROJECT}")).await;
    let operators = listed_by(&guarded, wiring::an_operator(), "/invitations").await;

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
    let guarded = guarded();
    issued_and_listed(&guarded).await;
    let at = format!("/invitations?project={PROJECT}");

    refused_as(
        &listed_by(&guarded, acting("account_grace", false), &at).await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OWNER,
    );
    refused_as(
        &listed_by(&guarded, acting("account_op", true), &at).await,
        StatusCode::NOT_FOUND,
        "not-found",
    );
}

#[tokio::test]
async fn an_owner_withdraws_a_project_s_invitation_which_nobody_else_may() {
    let guarded = guarded();
    let (into_the_project, _) = issued_and_listed(&guarded).await;
    let at = format!("/invitations/{into_the_project}");

    refused_as(
        &served_to(&guarded, acting("account_grace", false))
            .delete(&at)
            .await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OWNER,
    );
    for stranger in [acting("account_cy", false), acting("account_op", true)] {
        refused_as(
            &served_to(&guarded, stranger).delete(&at).await,
            StatusCode::NOT_FOUND,
            refused::INVITATION_NOT_FOUND,
        );
    }
    served_to(&guarded, ada())
        .delete(&at)
        .await
        .assert_status(StatusCode::NO_CONTENT);

    assert!(matches!(
        told(&guarded.wired, &into_the_project).await.as_slice(),
        [InvitationEvent::Issued { .. }, InvitationEvent::Withdrawn]
    ));
}
