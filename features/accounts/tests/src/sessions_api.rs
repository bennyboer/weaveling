use accounts_contract::{SESSION_COOKIE, SignInRequest, SignedInDTO, refused, stage};
use accounts_core::{SessionToken, Sessions};
use axum::http::StatusCode;
use axum::http::header::{COOKIE, RETRY_AFTER, SET_COOKIE};
use axum_test::{TestResponse, TestServer};
use serde_json::Value;
use time::Duration;

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

async fn served_with_ada_verified() -> (Wired, TestServer) {
    let wired = wiring::wired();
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    wired
        .accounts
        .verify(ADA, &wired.last_code_sent_to(ADA))
        .await
        .expect("the code sent should verify");
    let server = TestServer::new(wired.routes.clone());

    (wired, server)
}

fn signing_in(email: &str, password: &str) -> SignInRequest {
    SignInRequest {
        email: email.to_owned(),
        password: password.to_owned(),
    }
}

fn set_cookie(response: &TestResponse) -> String {
    response
        .header(SET_COOKIE)
        .to_str()
        .expect("a cookie is plain text")
        .to_owned()
}

fn token_in(set_cookie: &str) -> String {
    set_cookie
        .split(';')
        .next()
        .and_then(|pair| pair.strip_prefix(&format!("{SESSION_COOKIE}=")))
        .expect("the cookie should carry the session")
        .to_owned()
}

async fn a_session_token(server: &TestServer) -> String {
    let response = server
        .post("/sessions")
        .json(&signing_in(ADA, HER_PASSWORD))
        .await;
    response.assert_status_ok();

    token_in(&set_cookie(&response))
}

async fn current_with(server: &TestServer, cookie: &str) -> TestResponse {
    server
        .get("/sessions/current")
        .add_header(COOKIE, cookie.to_owned())
        .await
}

fn refused_as(response: &TestResponse, status: StatusCode, kind: &str) {
    response.assert_status(status);
    let told: Value = response.json();
    assert_eq!(told["type"], kind, "{told}");
}

#[tokio::test]
async fn signing_in_answers_who_signed_in() {
    // given: a verified account
    let (_wired, server) = served_with_ada_verified().await;

    // when: she signs in
    let response = server
        .post("/sessions")
        .json(&signing_in(ADA, HER_PASSWORD))
        .await;

    // then: the answer names her
    response.assert_status_ok();
    let signed_in: SignedInDTO = response.json();
    assert_eq!(signed_in.name, "Ada");
    assert_eq!(signed_in.email, ADA);
    assert_eq!(signed_in.stage, stage::SIGNED_IN);
}

#[tokio::test]
async fn the_session_cookie_is_out_of_reach_of_scripts_and_other_sites() {
    // given: a verified account
    let (_wired, server) = served_with_ada_verified().await;

    // when: she signs in
    let response = server
        .post("/sessions")
        .json(&signing_in(ADA, HER_PASSWORD))
        .await;

    // then: the cookie is kept from scripts and other sites
    let cookie = set_cookie(&response);
    for attribute in [
        "HttpOnly",
        "Secure",
        "SameSite=Strict",
        "Path=/",
        "Max-Age=2592000",
    ] {
        assert!(
            cookie.split("; ").any(|part| part == attribute),
            "{attribute} is missing from {cookie}"
        );
    }
    // and: it carries a long token
    assert_eq!(token_in(&cookie).len(), 64);
}

#[tokio::test]
async fn the_cookie_names_who_is_signed_in_later() {
    // given: a verified account
    let (_wired, server) = served_with_ada_verified().await;
    // and: a session of hers
    let signed_in: SignedInDTO = server
        .post("/sessions")
        .json(&signing_in(ADA, HER_PASSWORD))
        .await
        .json();
    let token = a_session_token(&server).await;

    // when: the current session is asked for with her cookie among others
    let current = current_with(
        &server,
        &format!("theme=dark; {SESSION_COOKIE}={token}; other=1"),
    )
    .await;

    // then: it names her
    current.assert_status_ok();
    let later: SignedInDTO = current.json();
    assert_eq!(later.account, signed_in.account);
    assert_eq!(later.email, ADA);
}

#[tokio::test]
async fn without_a_session_nobody_is_signed_in() {
    // given: a verified account
    let (_wired, server) = served_with_ada_verified().await;

    // then: without a cookie nobody is signed in
    refused_as(
        &server.get("/sessions/current").await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
    // and: neither does a token no session holds
    refused_as(
        &current_with(
            &server,
            &format!("{SESSION_COOKIE}={}", SessionToken::drawn().as_str()),
        )
        .await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
}

#[tokio::test]
async fn a_wrong_password_signs_nobody_in_and_sets_no_cookie() {
    // given: a verified account
    let (_wired, server) = served_with_ada_verified().await;

    // when: someone signs in with a wrong password
    let response = server
        .post("/sessions")
        .json(&signing_in(ADA, "not her password at all"))
        .await;

    // then: signing in is refused
    refused_as(
        &response,
        StatusCode::UNAUTHORIZED,
        refused::SIGN_IN_REFUSED,
    );
    // and: no cookie is set
    assert!(response.maybe_header(SET_COOKIE).is_none());
}

#[tokio::test]
async fn an_unverified_address_is_forbidden_until_verified() {
    // given: an account whose address is not verified
    let wired = wiring::wired();
    wired
        .accounts
        .sign_up(ADA, "Ada", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    let server = TestServer::new(wired.routes.clone());

    // then: signing in with it is forbidden
    refused_as(
        &server
            .post("/sessions")
            .json(&signing_in(ADA, HER_PASSWORD))
            .await,
        StatusCode::FORBIDDEN,
        refused::NOT_VERIFIED,
    );
}

#[tokio::test]
async fn signing_out_ends_the_session_and_lets_the_cookie_go() {
    // given: a signed in account
    let (_wired, server) = served_with_ada_verified().await;
    let token = a_session_token(&server).await;
    let cookie = format!("{SESSION_COOKIE}={token}");

    // when: she signs out
    let response = server
        .delete("/sessions/current")
        .add_header(COOKIE, cookie.clone())
        .await;

    // then: the cookie is let go
    response.assert_status(StatusCode::NO_CONTENT);
    assert!(set_cookie(&response).contains("Max-Age=0"));
    // and: the session no longer signs her in
    refused_as(
        &current_with(&server, &cookie).await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
}

#[tokio::test]
async fn signing_out_on_one_device_leaves_another_signed_in() {
    // given: an account signed in on two devices
    let (_wired, server) = served_with_ada_verified().await;
    let phone = a_session_token(&server).await;
    let laptop = a_session_token(&server).await;

    // when: she signs out on one
    server
        .delete("/sessions/current")
        .add_header(COOKIE, format!("{SESSION_COOKIE}={phone}"))
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: the other is still signed in
    current_with(&server, &format!("{SESSION_COOKIE}={laptop}"))
        .await
        .assert_status_ok();
}

#[tokio::test]
async fn signing_out_without_a_session_is_harmless() {
    // given: a verified account
    let (_wired, server) = served_with_ada_verified().await;

    // then: signing out without a session succeeds
    server
        .delete("/sessions/current")
        .await
        .assert_status(StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn a_session_is_over_after_thirty_days_and_ended_once_found_so() {
    // given: a signed in account
    let (wired, server) = served_with_ada_verified().await;
    let token = a_session_token(&server).await;
    let cookie = format!("{SESSION_COOKIE}={token}");

    // when: thirty days pass
    wired
        .clock
        .set(clock::at(1_700_000_000) + Duration::days(30));
    // then: she is still signed in
    current_with(&server, &cookie).await.assert_status_ok();

    // when: a second more passes
    wired
        .clock
        .set(clock::at(1_700_000_000) + Duration::days(30) + Duration::seconds(1));
    // then: she is signed in no longer
    refused_as(
        &current_with(&server, &cookie).await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
    // and: the session is ended
    assert_eq!(
        wired
            .secrets
            .find(&SessionToken::presented(&token).digest())
            .await
            .expect("finding should succeed"),
        None,
        "a session found over is ended, not kept waiting for a sweep"
    );
}

#[tokio::test]
async fn too_many_wrong_passwords_are_answered_with_how_long_to_wait() {
    // given: a verified account
    let (_wired, server) = served_with_ada_verified().await;
    // and: five wrong passwords for it
    for _ in 0..5 {
        refused_as(
            &server
                .post("/sessions")
                .json(&signing_in(ADA, "not her password at all"))
                .await,
            StatusCode::UNAUTHORIZED,
            refused::SIGN_IN_REFUSED,
        );
    }

    // when: she signs in with the right one
    let response = server
        .post("/sessions")
        .json(&signing_in(ADA, HER_PASSWORD))
        .await;

    // then: she is told how long to wait
    refused_as(&response, StatusCode::TOO_MANY_REQUESTS, refused::THROTTLED);
    assert_eq!(response.header(RETRY_AFTER), "60");
    assert_eq!(response.json::<Value>()["minutes"], 1);
    // and: no cookie is set
    assert!(response.maybe_header(SET_COOKIE).is_none());
}

#[tokio::test]
async fn a_wait_part_way_through_a_second_is_rounded_up() {
    // given: a verified account
    let (wired, server) = served_with_ada_verified().await;
    // and: five wrong passwords for it
    for _ in 0..5 {
        server
            .post("/sessions")
            .json(&signing_in(ADA, "not her password at all"))
            .await;
    }
    // and: half a second gone since
    wired
        .clock
        .set(clock::at(1_700_000_000) + Duration::milliseconds(500));

    // when: she signs in again
    let response = server
        .post("/sessions")
        .json(&signing_in(ADA, HER_PASSWORD))
        .await;

    // then: the wait is told in whole seconds rounded up
    assert_eq!(
        response.header(RETRY_AFTER),
        "60",
        "59.5 seconds told as 59 would have the client knock too early"
    );
    assert_eq!(response.json::<Value>()["minutes"], 1);
}
