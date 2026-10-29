use accounts_contract::{
    DeclineInvitationRequest, SignInRequest, TakeUpInvitationRequest, refused,
};
use accounts_core::{
    AccountEvent, DeclineReason, EmailAddress, EmailAddresses, InvitationScope, InvitationTicket,
    InvitationTickets, InvitationToken, KIND,
};
use axum::http::StatusCode;
use axum::http::header::{CONTENT_TYPE, COOKIE, SET_COOKIE};
use axum_test::{TestResponse, TestServer};
use eventsourcing::{AggregateId, EventStore, Version};
use serde_json::Value;
use time::Duration;

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

const TOKEN: &str = "a token drawn by invitations";
const BOB: &str = "bob@example.com";

async fn served_with_ada_invited() -> (Wired, TestServer) {
    let wired = wiring::wired();
    wired
        .secrets
        .keep_ticket(
            &InvitationToken::presented(TOKEN).digest(),
            &InvitationTicket {
                invitation: "invitation_1".to_owned(),
                to: EmailAddress::new(ADA).expect("a plain address"),
                until: wiring::at(1_700_000_000) + Duration::days(7),
                scope: InvitationScope::Project {
                    project: "project_loom".to_owned(),
                },
                by_an_operator: false,
            },
        )
        .await
        .expect("keeping should succeed");
    let server = TestServer::new(wired.routes.clone().merge(
        accounts_rest::admitting_only_the_signed_in(
            wired.accounts.clone(),
            accounts_rest::guarded_router(wired.accounts.clone()),
        ),
    ));

    (wired, server)
}

async fn signed_in(wired: &Wired, server: &TestServer, address: &str) -> String {
    wired
        .accounts
        .sign_up(address, "Someone", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    wired
        .accounts
        .verify(address, &wired.last_code_sent_to(address))
        .await
        .expect("the code sent should verify");

    server
        .post("/sessions")
        .json(&SignInRequest {
            email: address.to_owned(),
            password: HER_PASSWORD.to_owned(),
        })
        .await
        .header(SET_COOKIE)
        .to_str()
        .expect("a cookie is plain text")
        .split(';')
        .next()
        .expect("the cookie names the session")
        .to_owned()
}

async fn takes_up(server: &TestServer, cookie: Option<&str>, token: &str) -> TestResponse {
    let request =
        server
            .post("/accounts/current/invitations/accept")
            .json(&TakeUpInvitationRequest {
                token: token.to_owned(),
            });

    match cookie {
        Some(cookie) => request.add_header(COOKIE, cookie.to_owned()).await,
        None => request.await,
    }
}

fn refused_as(response: &TestResponse, status: StatusCode, kind: &str) -> Value {
    response.assert_status(status);
    assert_eq!(response.header(CONTENT_TYPE), "application/problem+json");
    let told = response.json::<Value>();
    assert_eq!(told["type"], kind);

    told
}

async fn taken_up_by(wired: &Wired, address: &str) -> bool {
    let holder = wired
        .secrets
        .holder(&EmailAddress::new(address).expect("a plain address"))
        .await
        .expect("reading should succeed")
        .expect("the address is held");

    wired
        .store
        .read_from(&AggregateId::from(&holder), KIND, Version::of(1))
        .await
        .expect("reading should succeed")
        .iter()
        .any(|recorded| matches!(recorded.event, AccountEvent::InvitationTakenUp { .. }))
}

#[tokio::test]
async fn the_account_signed_in_takes_up_an_invitation_to_its_address() {
    let (wired, server) = served_with_ada_invited().await;
    let ada = signed_in(&wired, &server, ADA).await;

    takes_up(&server, Some(&ada), TOKEN)
        .await
        .assert_status(StatusCode::NO_CONTENT);

    assert!(taken_up_by(&wired, ADA).await);
    refused_as(
        &takes_up(&server, Some(&ada), TOKEN).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::INVITATION_INVALID,
    );
}

#[tokio::test]
async fn an_invitation_to_another_address_is_refused_naming_it() {
    let (wired, server) = served_with_ada_invited().await;
    let bob = signed_in(&wired, &server, BOB).await;

    let told = refused_as(
        &takes_up(&server, Some(&bob), TOKEN).await,
        StatusCode::FORBIDDEN,
        refused::INVITATION_FOR_ANOTHER_ADDRESS,
    );

    assert_eq!(
        told["to"], ADA,
        "the page offers to sign in as the address invited"
    );
    assert!(!taken_up_by(&wired, BOB).await);
}

#[tokio::test]
async fn nobody_takes_an_invitation_up_without_signing_in() {
    let (_, server) = served_with_ada_invited().await;

    refused_as(
        &takes_up(&server, None, TOKEN).await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
}

async fn takes_up_by_id(
    server: &TestServer,
    cookie: Option<&str>,
    invitation: &str,
) -> TestResponse {
    let request = server.post(&format!(
        "/accounts/current/invitations/{invitation}/accept"
    ));

    match cookie {
        Some(cookie) => request.add_header(COOKIE, cookie.to_owned()).await,
        None => request.await,
    }
}

#[tokio::test]
async fn the_account_signed_in_takes_up_an_invitation_to_its_address_by_its_id() {
    let (wired, server) = served_with_ada_invited().await;
    let ada = signed_in(&wired, &server, ADA).await;

    takes_up_by_id(&server, Some(&ada), "invitation_1")
        .await
        .assert_status(StatusCode::NO_CONTENT);

    assert!(taken_up_by(&wired, ADA).await);
    refused_as(
        &takes_up_by_id(&server, Some(&ada), "invitation_1").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::INVITATION_INVALID,
    );
}

#[tokio::test]
async fn nobody_takes_an_invitation_up_by_its_id_without_signing_in_nor_one_to_another_address() {
    let (wired, server) = served_with_ada_invited().await;
    let bob = signed_in(&wired, &server, BOB).await;

    refused_as(
        &takes_up_by_id(&server, None, "invitation_1").await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
    refused_as(
        &takes_up_by_id(&server, Some(&bob), "invitation_1").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::INVITATION_INVALID,
    );
    assert!(!taken_up_by(&wired, BOB).await);
}

async fn declines(server: &TestServer, cookie: Option<&str>, token: &str) -> TestResponse {
    let request =
        server
            .post("/accounts/current/invitations/decline")
            .json(&DeclineInvitationRequest {
                token: token.to_owned(),
            });

    match cookie {
        Some(cookie) => request.add_header(COOKIE, cookie.to_owned()).await,
        None => request.await,
    }
}

async fn declines_by_id(
    server: &TestServer,
    cookie: Option<&str>,
    invitation: &str,
) -> TestResponse {
    let request = server.post(&format!(
        "/accounts/current/invitations/{invitation}/decline"
    ));

    match cookie {
        Some(cookie) => request.add_header(COOKIE, cookie.to_owned()).await,
        None => request.await,
    }
}

async fn turned_down_by(wired: &Wired, address: &str) -> bool {
    let holder = wired
        .secrets
        .holder(&EmailAddress::new(address).expect("a plain address"))
        .await
        .expect("reading should succeed")
        .expect("the address is held");

    wired
        .store
        .read_from(&AggregateId::from(&holder), KIND, Version::of(1))
        .await
        .expect("reading should succeed")
        .iter()
        .any(|recorded| {
            matches!(
                recorded.event,
                AccountEvent::InvitationDeclined {
                    reason: DeclineReason::TurnedDown,
                    ..
                }
            )
        })
}

#[tokio::test]
async fn the_account_signed_in_turns_down_an_invitation_to_its_address_by_the_link() {
    let (wired, server) = served_with_ada_invited().await;
    let ada = signed_in(&wired, &server, ADA).await;

    declines(&server, Some(&ada), TOKEN)
        .await
        .assert_status(StatusCode::NO_CONTENT);

    assert!(turned_down_by(&wired, ADA).await);
    refused_as(
        &takes_up(&server, Some(&ada), TOKEN).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::INVITATION_INVALID,
    );
}

#[tokio::test]
async fn the_account_signed_in_turns_down_an_invitation_to_its_address_by_its_id() {
    let (wired, server) = served_with_ada_invited().await;
    let ada = signed_in(&wired, &server, ADA).await;

    declines_by_id(&server, Some(&ada), "invitation_1")
        .await
        .assert_status(StatusCode::NO_CONTENT);

    assert!(turned_down_by(&wired, ADA).await);
    refused_as(
        &declines_by_id(&server, Some(&ada), "invitation_1").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::INVITATION_INVALID,
    );
}

#[tokio::test]
async fn nobody_turns_an_invitation_down_without_signing_in_nor_one_to_another_address() {
    let (wired, server) = served_with_ada_invited().await;
    let bob = signed_in(&wired, &server, BOB).await;

    for unsigned in [
        declines(&server, None, TOKEN).await,
        declines_by_id(&server, None, "invitation_1").await,
    ] {
        refused_as(&unsigned, StatusCode::UNAUTHORIZED, refused::NOT_SIGNED_IN);
    }
    refused_as(
        &declines_by_id(&server, Some(&bob), "invitation_1").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::INVITATION_INVALID,
    );
    assert!(!turned_down_by(&wired, BOB).await);
}
