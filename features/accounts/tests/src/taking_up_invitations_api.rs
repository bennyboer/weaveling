use accounts_contract::{
    DeclineInvitationRequest, SignInRequest, TakeUpInvitationRequest, refused,
};
use accounts_core::{
    AccountEvent, DeclineReason, EmailAddress, EmailAddresses, InvitationScope, InvitationTicket,
    InvitationToken, KIND,
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
    // given: an invitation into a project
    let (wired, server) = served_with_ada_invited().await;
    // and: its address signed in
    let ada = signed_in(&wired, &server, ADA).await;

    // when: she takes it up by its link
    takes_up(&server, Some(&ada), TOKEN)
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: her account took it up
    assert!(taken_up_by(&wired, ADA).await);
    // and: taking it up again is refused
    refused_as(
        &takes_up(&server, Some(&ada), TOKEN).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::INVITATION_INVALID,
    );
}

#[tokio::test]
async fn an_invitation_to_another_address_is_refused_naming_it() {
    // given: an invitation into a project
    let (wired, server) = served_with_ada_invited().await;
    // and: another address signed in
    let bob = signed_in(&wired, &server, BOB).await;

    // then: taking the invitation up by its link is refused as for another address
    let told = refused_as(
        &takes_up(&server, Some(&bob), TOKEN).await,
        StatusCode::FORBIDDEN,
        refused::INVITATION_FOR_ANOTHER_ADDRESS,
    );

    // and: the address invited is named
    assert_eq!(
        told["to"], ADA,
        "the page offers to sign in as the address invited"
    );
    // and: nothing is taken up
    assert!(!taken_up_by(&wired, BOB).await);
}

#[tokio::test]
async fn nobody_takes_an_invitation_up_without_signing_in() {
    // given: an invitation into a project
    let (_, server) = served_with_ada_invited().await;

    // then: taking it up without signing in is refused
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
    // given: an invitation into a project
    let (wired, server) = served_with_ada_invited().await;
    // and: its address signed in
    let ada = signed_in(&wired, &server, ADA).await;

    // when: she takes it up by its id
    takes_up_by_id(&server, Some(&ada), "invitation_1")
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: her account took it up
    assert!(taken_up_by(&wired, ADA).await);
    // and: taking it up again is refused
    refused_as(
        &takes_up_by_id(&server, Some(&ada), "invitation_1").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::INVITATION_INVALID,
    );
}

#[tokio::test]
async fn nobody_takes_an_invitation_up_by_its_id_without_signing_in_nor_one_to_another_address() {
    // given: an invitation into a project
    let (wired, server) = served_with_ada_invited().await;
    // and: another address signed in
    let bob = signed_in(&wired, &server, BOB).await;

    // then: taking it up by its id without signing in is refused
    refused_as(
        &takes_up_by_id(&server, None, "invitation_1").await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
    // and: so is taking it up by its id as the other address
    refused_as(
        &takes_up_by_id(&server, Some(&bob), "invitation_1").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::INVITATION_INVALID,
    );
    // and: nothing is taken up
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
    // given: an invitation into a project
    let (wired, server) = served_with_ada_invited().await;
    // and: its address signed in
    let ada = signed_in(&wired, &server, ADA).await;

    // when: she turns it down by its link
    declines(&server, Some(&ada), TOKEN)
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: her account turned it down
    assert!(turned_down_by(&wired, ADA).await);
    // and: taking it up is refused
    refused_as(
        &takes_up(&server, Some(&ada), TOKEN).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::INVITATION_INVALID,
    );
}

#[tokio::test]
async fn the_account_signed_in_turns_down_an_invitation_to_its_address_by_its_id() {
    // given: an invitation into a project
    let (wired, server) = served_with_ada_invited().await;
    // and: its address signed in
    let ada = signed_in(&wired, &server, ADA).await;

    // when: she turns it down by its id
    declines_by_id(&server, Some(&ada), "invitation_1")
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: her account turned it down
    assert!(turned_down_by(&wired, ADA).await);
    // and: turning it down again is refused
    refused_as(
        &declines_by_id(&server, Some(&ada), "invitation_1").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::INVITATION_INVALID,
    );
}

#[tokio::test]
async fn nobody_turns_an_invitation_down_without_signing_in_nor_one_to_another_address() {
    // given: an invitation into a project
    let (wired, server) = served_with_ada_invited().await;
    // and: another address signed in
    let bob = signed_in(&wired, &server, BOB).await;

    // then: turning it down without signing in is refused
    for unsigned in [
        declines(&server, None, TOKEN).await,
        declines_by_id(&server, None, "invitation_1").await,
    ] {
        refused_as(&unsigned, StatusCode::UNAUTHORIZED, refused::NOT_SIGNED_IN);
    }
    // and: so is turning it down as the other address
    refused_as(
        &declines_by_id(&server, Some(&bob), "invitation_1").await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::INVITATION_INVALID,
    );
    // and: nothing is turned down
    assert!(!turned_down_by(&wired, BOB).await);
}

#[tokio::test]
async fn an_invitation_into_a_project_in_the_trash_is_refused_as_waiting() {
    // given: an invitation into a project
    let (wired, server) = served_with_ada_invited().await;
    // and: its address signed in
    let ada = signed_in(&wired, &server, ADA).await;
    // and: the project in the trash
    wired
        .accounts
        .remember_trash(None, "project_loom", 5, true)
        .await
        .expect("remembering should succeed");

    // when: she takes the invitation up
    let response = takes_up(&server, Some(&ada), TOKEN).await;

    // then: it is refused as waiting
    refused_as(&response, StatusCode::CONFLICT, refused::INVITATION_PAUSED);
    // and: nothing is taken up
    assert!(!taken_up_by(&wired, ADA).await);
}
