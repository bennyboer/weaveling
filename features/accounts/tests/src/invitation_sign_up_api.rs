use accounts_contract::SignUpRequest;
use accounts_contract::{SESSION_COOKIE, SignUpByInvitationRequest, SignedInDTO, refused, stage};
use accounts_core::{
    EmailAddress, InvitationTicket, InvitationTickets, InvitationToken, SigningUp,
};
use axum::http::StatusCode;
use axum::http::header::{CONTENT_TYPE, SET_COOKIE};
use axum_test::{TestResponse, TestServer};
use serde_json::Value;
use time::Duration;

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

const TOKEN: &str = "a token drawn by invitations";

async fn invited() -> (Wired, TestServer) {
    let wired = wiring::wired();
    wired
        .secrets
        .keep_ticket(
            &InvitationToken::presented(TOKEN).digest(),
            &InvitationTicket {
                invitation: "invitation_1".to_owned(),
                to: EmailAddress::new(ADA).expect("a plain address"),
                until: wiring::at(1_700_000_000) + Duration::days(7),
            },
        )
        .await
        .expect("keeping should succeed");
    let server = TestServer::new(wired.routes.clone());

    (wired, server)
}

async fn signs_up(server: &TestServer, token: &str) -> TestResponse {
    server
        .post("/accounts/by-invitation")
        .json(&SignUpByInvitationRequest {
            token: token.to_owned(),
            name: "Ada".to_owned(),
            password: HER_PASSWORD.to_owned(),
        })
        .await
}

fn refused_as(response: TestResponse, status: StatusCode, kind: &str) {
    response.assert_status(status);
    assert_eq!(response.header(CONTENT_TYPE), "application/problem+json");
    assert_eq!(response.json::<Value>()["type"], kind);
}

#[tokio::test]
async fn an_invitation_makes_an_account_and_opens_its_session() {
    let (wired, server) = invited().await;

    let response = signs_up(&server, TOKEN).await;

    response.assert_status(StatusCode::CREATED);
    let signed_in: SignedInDTO = response.json();
    assert_eq!(
        (
            signed_in.name.as_str(),
            signed_in.email.as_str(),
            signed_in.stage.as_str()
        ),
        ("Ada", ADA, stage::SIGNED_IN)
    );
    let cookie = response
        .maybe_header(SET_COOKIE)
        .expect("a session is opened")
        .to_str()
        .expect("a cookie is plain text")
        .to_owned();
    assert!(
        cookie.starts_with(&format!("{SESSION_COOKIE}=")),
        "{cookie}"
    );
    assert!(cookie.contains("HttpOnly"), "{cookie}");
    wired
        .accounts
        .sign_in(ADA, HER_PASSWORD)
        .await
        .expect("the address is confirmed already");
}

#[tokio::test]
async fn a_link_that_leads_nowhere_is_refused_by_its_kind() {
    let (_wired, server) = invited().await;

    refused_as(
        signs_up(&server, "a token nobody was given").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::INVITATION_INVALID,
    );
}

#[tokio::test]
async fn a_link_run_out_is_refused_by_its_kind() {
    let (wired, server) = invited().await;
    wired
        .clock
        .set(wiring::at(1_700_000_000) + Duration::days(8));

    refused_as(
        signs_up(&server, TOKEN).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::INVITATION_EXPIRED,
    );
}

#[tokio::test]
async fn an_address_with_an_account_already_is_refused_by_its_kind() {
    let (wired, server) = invited().await;
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");

    refused_as(
        signs_up(&server, TOKEN).await,
        StatusCode::CONFLICT,
        refused::EMAIL_ADDRESS_HAS_ACCOUNT,
    );
}

#[tokio::test]
async fn a_sign_up_with_the_operator_code_is_let_in_and_a_wrong_code_refused_by_its_kind() {
    let wired = wiring::wired();
    let code = wired
        .accounts
        .draw_operator_code()
        .await
        .expect("drawing should succeed")
        .expect("with nobody appointed a code is drawn");
    let server = TestServer::new(accounts_rest::router(
        wired.accounts.clone().signing_up(SigningUp::ByInvitation),
    ));
    let signing_up = |code: &str| SignUpRequest {
        email: ADA.to_owned(),
        name: "Ada".to_owned(),
        password: HER_PASSWORD.to_owned(),
        operator_code: Some(code.to_owned()),
    };

    refused_as(
        server
            .post("/accounts")
            .json(&signing_up("0123456789abcdef0123456789abcdef"))
            .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::OPERATOR_CODE_WRONG,
    );
    server
        .post("/accounts")
        .json(&signing_up(code.as_str()))
        .await
        .assert_status(StatusCode::ACCEPTED);
}

#[tokio::test]
async fn a_plain_sign_up_where_signing_up_is_by_invitation_only_is_refused_by_its_kind() {
    let wired = wiring::wired();
    let server = TestServer::new(accounts_rest::router(
        wired.accounts.clone().signing_up(SigningUp::ByInvitation),
    ));

    refused_as(
        server
            .post("/accounts")
            .json(&SignUpRequest {
                email: ADA.to_owned(),
                name: "Ada".to_owned(),
                password: HER_PASSWORD.to_owned(),
                operator_code: None,
            })
            .await,
        StatusCode::FORBIDDEN,
        refused::BY_INVITATION_ONLY,
    );
}
