use accounts_contract::SignUpRequest;
use accounts_contract::{SESSION_COOKIE, SignUpByInvitationRequest, SignedInDTO, refused, stage};
use accounts_core::{EmailAddress, InvitationScope, InvitationTicket, InvitationToken, SigningUp};
use axum::http::StatusCode;
use axum::http::header::{CONTENT_TYPE, SET_COOKIE};
use axum_test::{TestResponse, TestServer};
use serde_json::Value;
use time::Duration;

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

const TOKEN: &str = "a token drawn by invitations";

async fn an_open_invitation() -> (Wired, TestServer) {
    let wired = wiring::wired();
    wired
        .keep_ticket(
            &InvitationToken::presented(TOKEN).digest(),
            &InvitationTicket {
                invitation: "invitation_1".to_owned(),
                to: EmailAddress::new(ADA).expect("a plain address"),
                until: clock::at(1_700_000_000) + Duration::days(7),
                scope: InvitationScope::Instance,
                by_an_operator: true,
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
    // given: an open invitation
    let (wired, server) = an_open_invitation().await;

    // when: someone signs up by its link
    let response = signs_up(&server, TOKEN).await;

    // then: an account is made under the invited address
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
    // and: its session is opened in a cookie scripts cannot read
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
    // and: the account signs in with its address confirmed
    wired
        .accounts
        .sign_in(ADA, HER_PASSWORD)
        .await
        .expect("the address is confirmed already");
}

#[tokio::test]
async fn a_link_that_leads_nowhere_is_refused_by_its_kind() {
    // given: an open invitation
    let (_wired, server) = an_open_invitation().await;

    // then: signing up by a link nobody was given is refused
    refused_as(
        signs_up(&server, "a token nobody was given").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::INVITATION_INVALID,
    );
}

#[tokio::test]
async fn a_link_run_out_is_refused_by_its_kind() {
    // given: an open invitation
    let (wired, server) = an_open_invitation().await;
    // and: its last moment past
    wired
        .clock
        .set(clock::at(1_700_000_000) + Duration::days(8));

    // then: signing up by its link is refused
    refused_as(
        signs_up(&server, TOKEN).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::INVITATION_EXPIRED,
    );
}

#[tokio::test]
async fn an_address_with_an_account_already_is_refused_by_its_kind() {
    // given: an open invitation
    let (wired, server) = an_open_invitation().await;
    // and: an account under the invited address already
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");

    // then: signing up by its link is refused
    refused_as(
        signs_up(&server, TOKEN).await,
        StatusCode::CONFLICT,
        refused::EMAIL_ADDRESS_HAS_ACCOUNT,
    );
}

#[tokio::test]
async fn a_sign_up_with_the_operator_code_is_let_in_and_a_wrong_code_refused_by_its_kind() {
    // given: an operator code drawn
    let wired = wiring::wired();
    let code = wired
        .accounts
        .draw_operator_code()
        .await
        .expect("drawing should succeed")
        .expect("with nobody appointed a code is drawn");
    // and: signing up by invitation only
    let server = TestServer::new(accounts_rest::router(
        wired.accounts.clone().signing_up(SigningUp::ByInvitation),
    ));
    let signing_up = |code: &str| SignUpRequest {
        email: ADA.to_owned(),
        name: "Ada".to_owned(),
        password: HER_PASSWORD.to_owned(),
        operator_code: Some(code.to_owned()),
    };

    // then: a sign up with a wrong code is refused
    refused_as(
        server
            .post("/accounts")
            .json(&signing_up("0123456789abcdef0123456789abcdef"))
            .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::OPERATOR_CODE_WRONG,
    );
    // and: one with the code drawn is let in
    server
        .post("/accounts")
        .json(&signing_up(code.as_str()))
        .await
        .assert_status(StatusCode::ACCEPTED);
}

#[tokio::test]
async fn a_plain_sign_up_where_signing_up_is_by_invitation_only_is_refused_by_its_kind() {
    // given: signing up by invitation only
    let wired = wiring::wired();
    let server = TestServer::new(accounts_rest::router(
        wired.accounts.clone().signing_up(SigningUp::ByInvitation),
    ));

    // then: a plain sign up is refused
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

#[tokio::test]
async fn an_owner_s_invitation_where_signing_up_is_by_invitation_only_is_refused_by_its_kind() {
    // given: an open invitation from an owner rather than an operator
    let wired = wiring::wired();
    wired
        .keep_ticket(
            &InvitationToken::presented(TOKEN).digest(),
            &InvitationTicket {
                invitation: "invitation_1".to_owned(),
                to: EmailAddress::new(ADA).expect("a plain address"),
                until: clock::at(1_700_000_000) + Duration::days(7),
                scope: InvitationScope::Project {
                    project: "project_loom".to_owned(),
                },
                by_an_operator: false,
            },
        )
        .await
        .expect("keeping should succeed");
    // and: signing up by invitation only
    let server = TestServer::new(accounts_rest::router(
        wired.accounts.clone().signing_up(SigningUp::ByInvitation),
    ));

    // then: signing up by its link is refused
    refused_as(
        signs_up(&server, TOKEN).await,
        StatusCode::FORBIDDEN,
        refused::INVITATION_NOT_BY_AN_OPERATOR,
    );
}
