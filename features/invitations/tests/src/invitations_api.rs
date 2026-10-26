use acting::Acting;
use axum::Extension;
use axum::http::StatusCode;
use axum::http::header::{ORIGIN, RETRY_AFTER};
use axum_test::{TestResponse, TestServer};
use eventsourcing::{Agent, AggregateId, EventStore, Version};
use invitations_contract::{InvitationSummaryDTO, InviteRequest, refused};
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
    let wired = wiring::wired();
    let server = served_to(&wired, wiring::an_operator());

    let response = invite(&server, ADA).await;

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
    assert!(wired.links_mailed_to(ADA)[0].starts_with("https://weaveling.example.com/invitation#"));
    let id: InvitationId = invited.id.parse().expect("the id told is an invitation's");
    let issued = wired
        .store
        .read_from(&AggregateId::from(&id), KIND, Version::of(1))
        .await
        .expect("reading should succeed");
    assert!(matches!(
        &issued[0].event,
        InvitationEvent::Issued { digest, .. } if *digest == wired.token_mailed_to(ADA).digest()
    ));
}

#[tokio::test]
async fn an_invitation_named_from_no_page_is_refused() {
    let wired = wiring::wired();
    let server = served_to(&wired, wiring::an_operator());

    refused_as(
        &server.post("/invitations").json(&inviting(ADA)).await,
        StatusCode::BAD_REQUEST,
        refused::ORIGIN_MISSING,
    );
    assert!(wired.mailbox.sent_to(ADA).is_empty());
}

#[tokio::test]
async fn anyone_but_an_operator_is_refused() {
    let wired = wiring::wired();
    let id = wired
        .invitations
        .issue(&wiring::an_operator(), ADA, INSTANCE)
        .await
        .expect("an operator may invite")
        .id;
    let server = served_to(&wired, wiring::no_operator());

    refused_as(
        &invite(&server, "grace@example.com").await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OPERATOR,
    );
    refused_as(
        &server.delete(&format!("/invitations/{id}")).await,
        StatusCode::FORBIDDEN,
        refused::NOT_AN_OPERATOR,
    );
}

#[tokio::test]
async fn an_address_is_refused_by_its_kind() {
    let wired = wiring::wired();
    let server = served_to(&wired, wiring::an_operator());

    refused_as(
        &invite(&server, "ada").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::EMAIL_MALFORMED,
    );
    let told = refused_as(
        &invite(&server, &format!("{}@example.com", "a".repeat(250))).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::EMAIL_TOO_LONG,
    );
    assert_eq!(told["max"], 254);
}

#[tokio::test]
async fn an_address_invited_too_often_is_told_how_long_to_wait() {
    let wired = wiring::wired();
    let server = served_to(&wired, wiring::an_operator());
    for _ in 0..3 {
        invite(&server, ADA)
            .await
            .assert_status(StatusCode::CREATED);
    }

    let response = invite(&server, ADA).await;

    let told = refused_as(&response, StatusCode::TOO_MANY_REQUESTS, refused::THROTTLED);
    assert_eq!(response.header(RETRY_AFTER), "60");
    assert_eq!(told["minutes"], 1);
}

#[tokio::test]
async fn an_operator_withdraws_and_an_invitation_missing_or_no_invitation_at_all_is_said_so() {
    let wired = wiring::wired();
    let id = wired
        .invitations
        .issue(&wiring::an_operator(), ADA, INSTANCE)
        .await
        .expect("an operator may invite")
        .id;
    let server = served_to(&wired, wiring::an_operator());

    server
        .delete(&format!("/invitations/{id}"))
        .await
        .assert_status(StatusCode::NO_CONTENT);
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
    refused_as(
        &server.delete("/invitations/not-an-id").await,
        StatusCode::BAD_REQUEST,
        refused::INVITATION_ID_INVALID,
    );
}

#[tokio::test]
async fn an_invitation_taken_up_is_not_withdrawn() {
    let wired = wiring::wired();
    let id = wired
        .invitations
        .issue(&wiring::an_operator(), ADA, INSTANCE)
        .await
        .expect("an operator may invite")
        .id;
    wired
        .invitations
        .taken_up(
            &id,
            wiring::at(1_700_000_100),
            &Agent::Account("account_ada".into()),
        )
        .await
        .expect("an open invitation is accepted");

    refused_as(
        &served_to(&wired, wiring::an_operator())
            .delete(&format!("/invitations/{id}"))
            .await,
        StatusCode::CONFLICT,
        refused::INVITATION_ALREADY_ACCEPTED,
    );
}
