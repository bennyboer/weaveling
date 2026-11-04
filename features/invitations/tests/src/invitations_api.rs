use acting::Acting;
use axum::Extension;
use axum::http::StatusCode;
use axum::http::header::{ORIGIN, RETRY_AFTER};
use axum_test::{TestResponse, TestServer};
use eventsourcing::{Agent, AggregateId, EventStore, Version};
use invitations_contract::{InvitationSummaryDTO, InviteRequest, ScopeDTO, refused};
use invitations_core::{InvitationEvent, InvitationId, KIND};
use serde_json::Value;

use crate::wiring;
use crate::wiring::{ORIGIN as INSTANCE, Wired};

const ADA: &str = "ada@example.com";

fn served_to(wired: &Wired, acting: Acting) -> TestServer {
    TestServer::new(invitations_rest::router(wired.invitations.clone()).layer(Extension(acting)))
}

fn inviting(email: &str) -> InviteRequest {
    InviteRequest {
        email: email.to_owned(),
        scope: ScopeDTO::Instance,
    }
}

async fn invite(server: &TestServer, email: &str) -> TestResponse {
    server
        .post("/invitations")
        .add_header(ORIGIN, INSTANCE)
        .json(&inviting(email))
        .await
}

fn refused_as(response: &TestResponse, status: StatusCode, kind: &str) -> Value {
    response.assert_status(status);
    let told: Value = response.json();
    assert_eq!(told["type"], kind, "{told}");

    told
}

#[tokio::test]
async fn an_operator_invites_and_the_link_is_made_from_the_page_it_was_sent_from() {
    // given: an operator
    let wired = wiring::a_workbench();
    let server = served_to(&wired, wiring::an_operator());

    // when: it invites
    let response = invite(&server, ADA).await;

    // then: the invitation is told as the list will show it
    response.assert_status(StatusCode::CREATED);
    let invited: InvitationSummaryDTO = response.json();
    assert_eq!(
        (
            invited.email.as_str(),
            invited.issued_at.as_str(),
            invited.until.as_str(),
            invited.status.as_str(),
            invited.declined_for,
        ),
        (
            ADA,
            "2023-11-14T22:13:20Z",
            "2023-11-21T22:13:20Z",
            "open",
            None
        ),
        "the answer is the invitation as the list will show it, so the page need not wait for the list"
    );
    // and: the link mailed is made from the page it was sent from
    assert!(
        wired.links_mailed_to(ADA).await[0]
            .starts_with("https://weaveling.example.com/invitation#")
    );
    // and: the invitation keeps the digest of the token mailed
    let id: InvitationId = invited.id.parse().expect("the id told is an invitation's");
    let issued = wired
        .store
        .read_from(&AggregateId::from(&id), KIND, Version::of(1))
        .await
        .expect("reading should succeed");
    assert!(matches!(
        &issued[0].event,
        InvitationEvent::Issued { digest, .. } if *digest == wired.token_mailed_to(ADA).await.digest()
    ));
}

#[tokio::test]
async fn an_invitation_named_from_no_page_is_refused() {
    // given: an operator
    let wired = wiring::a_workbench();
    let server = served_to(&wired, wiring::an_operator());

    // then: inviting from no page is refused
    refused_as(
        &server.post("/invitations").json(&inviting(ADA)).await,
        StatusCode::BAD_REQUEST,
        refused::ORIGIN_MISSING,
    );
    // and: nothing is mailed
    assert!(wired.sent_to(ADA).await.is_empty());
}

#[tokio::test]
async fn anyone_but_an_operator_is_refused() {
    // given: an open invitation
    let wired = wiring::a_workbench();
    let id = wired
        .invitations
        .invite_to_instance(&wiring::an_operator(), ADA, INSTANCE)
        .await
        .expect("an operator may invite")
        .id;
    // and: someone who is no operator
    let server = served_to(&wired, wiring::no_operator());

    // then: inviting is refused
    refused_as(
        &invite(&server, "grace@example.com").await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OPERATOR,
    );
    // and: withdrawing is refused
    refused_as(
        &server.delete(&format!("/invitations/{id}")).await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OPERATOR,
    );
}

#[tokio::test]
async fn an_address_is_refused_by_its_kind() {
    // given: an operator
    let wired = wiring::a_workbench();
    let server = served_to(&wired, wiring::an_operator());

    // then: an address that is none is refused as malformed
    refused_as(
        &invite(&server, "ada").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::EMAIL_MALFORMED,
    );
    // and: one too long is refused as too long, telling the most allowed
    let told = refused_as(
        &invite(&server, &format!("{}@example.com", "a".repeat(250))).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::EMAIL_TOO_LONG,
    );
    assert_eq!(told["max"], 254);
}

#[tokio::test]
async fn an_address_invited_too_often_is_told_how_long_to_wait() {
    // given: an address invited as often as allowed
    let wired = wiring::a_workbench();
    let server = served_to(&wired, wiring::an_operator());
    for _ in 0..3 {
        invite(&server, ADA)
            .await
            .assert_status(StatusCode::CREATED);
    }

    // when: it is invited once more
    let response = invite(&server, ADA).await;

    // then: it is refused, telling how long to wait
    let told = refused_as(&response, StatusCode::TOO_MANY_REQUESTS, refused::THROTTLED);
    assert_eq!(response.header(RETRY_AFTER), "60");
    assert_eq!(told["minutes"], 1);
}

#[tokio::test]
async fn an_operator_withdraws_and_an_invitation_missing_or_no_invitation_at_all_is_said_so() {
    // given: an open invitation
    let wired = wiring::a_workbench();
    let id = wired
        .invitations
        .invite_to_instance(&wiring::an_operator(), ADA, INSTANCE)
        .await
        .expect("an operator may invite")
        .id;
    // and: an operator
    let server = served_to(&wired, wiring::an_operator());

    // then: it withdraws the invitation
    server
        .delete(&format!("/invitations/{id}"))
        .await
        .assert_status(StatusCode::NO_CONTENT);
    // and: one missing is refused as not found
    refused_as(
        &server
            .delete(&format!(
                "/invitations/{}",
                InvitationId::generate(wiring::at(1_000))
            ))
            .await,
        StatusCode::NOT_FOUND,
        refused::INVITATION_NOT_FOUND,
    );
    // and: no invitation at all is refused as invalid
    refused_as(
        &server.delete("/invitations/not-an-id").await,
        StatusCode::BAD_REQUEST,
        refused::INVITATION_ID_INVALID,
    );
}

#[tokio::test]
async fn an_invitation_taken_up_is_not_withdrawn() {
    // given: an invitation taken up
    let wired = wiring::a_workbench();
    let id = wired
        .invitations
        .invite_to_instance(&wiring::an_operator(), ADA, INSTANCE)
        .await
        .expect("an operator may invite")
        .id;
    wired
        .invitations
        .taken_up(None, &id, &Agent::Account("account_ada".into()))
        .await
        .expect("an open invitation is accepted");

    // then: withdrawing it is refused as accepted already
    refused_as(
        &served_to(&wired, wiring::an_operator())
            .delete(&format!("/invitations/{id}"))
            .await,
        StatusCode::CONFLICT,
        refused::INVITATION_ALREADY_ACCEPTED,
    );
}
