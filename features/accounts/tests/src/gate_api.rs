use accounts_contract::{SESSION_COOKIE, SignInRequest, refused};
use accounts_core::{AccountId, Session, SessionToken, Stage};
use acting::Acting;
use axum::http::StatusCode;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::routing;
use axum::{Extension, Router};
use axum_test::{TestResponse, TestServer};
use clock::Clock;
use eventsourcing::Agent;
use serde_json::Value;

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

async fn who_acts(Extension(acting): Extension<Agent>) -> String {
    acting.to_string()
}

async fn how_it_acts(Extension(acting): Extension<Acting>) -> String {
    format!("{} operator: {}", acting.agent, acting.operator)
}

async fn behind_the_gate() -> (Wired, TestServer, TestServer) {
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
    let work = accounts_rest::admitting_only_the_signed_in(
        wired.accounts.clone(),
        Router::new()
            .route("/work", routing::get(who_acts))
            .route("/acting", routing::get(how_it_acts)),
    );

    let accounts = TestServer::new(wired.routes.clone());

    (wired, accounts, TestServer::new(work))
}

async fn a_session_cookie(accounts: &TestServer) -> String {
    let response = accounts
        .post("/sessions")
        .json(&SignInRequest {
            email: ADA.to_owned(),
            password: HER_PASSWORD.to_owned(),
        })
        .await;
    response.assert_status_ok();

    response
        .header(SET_COOKIE)
        .to_str()
        .expect("a cookie is plain text")
        .split(';')
        .next()
        .expect("the cookie names the session")
        .to_owned()
}

fn not_signed_in(response: &TestResponse) {
    response.assert_status(StatusCode::UNAUTHORIZED);
    let told: Value = response.json();
    assert_eq!(told["type"], refused::NOT_SIGNED_IN, "{told}");
}

#[tokio::test]
async fn the_work_behind_the_gate_is_done_by_whoever_signed_in() {
    // given: work behind the gate
    let (wired, accounts, work) = behind_the_gate().await;
    // and: a session of a signed in account
    let cookie = a_session_cookie(&accounts).await;
    let ada = wired
        .accounts
        .sign_in(ADA, HER_PASSWORD)
        .await
        .expect("the right pair should sign in");

    // when: the work is asked for with that session
    let response = work.get("/work").add_header(COOKIE, cookie).await;

    // then: it is done by that account
    response.assert_status_ok();
    assert_eq!(response.text(), ada.acting().to_string());
}

#[tokio::test]
async fn without_a_session_the_gate_stays_shut() {
    // given: work behind the gate
    let (_wired, _accounts, work) = behind_the_gate().await;

    // then: it is refused without a session
    not_signed_in(&work.get("/work").await);
    // and: with a session nobody opened
    not_signed_in(
        &work
            .get("/work")
            .add_header(
                COOKIE,
                format!("{SESSION_COOKIE}={}", SessionToken::drawn().as_str()),
            )
            .await,
    );
}

#[tokio::test]
async fn a_session_still_owing_its_second_factor_does_not_pass() {
    // given: work behind the gate
    let (wired, _accounts, work) = behind_the_gate().await;
    // and: a session still owing its second factor
    let token = SessionToken::drawn();
    let id: AccountId = wired
        .accounts
        .sign_in(ADA, HER_PASSWORD)
        .await
        .expect("the right pair should sign in");
    wired
        .open(
            &token.digest(),
            &Session::begun(id, Stage::SecondFactorOwed, wired.clock.now()),
        )
        .await
        .expect("opening should succeed");

    // then: the work refuses it
    not_signed_in(
        &work
            .get("/work")
            .add_header(COOKIE, format!("{SESSION_COOKIE}={}", token.as_str()))
            .await,
    );
}

#[tokio::test]
async fn the_work_is_told_whether_whoever_acts_is_an_operator_as_it_now_stands() {
    // given: work behind the gate
    let (wired, accounts, work) = behind_the_gate().await;
    // and: a session of an account that is no operator
    let cookie = a_session_cookie(&accounts).await;
    let ada = wired
        .accounts
        .sign_in(ADA, HER_PASSWORD)
        .await
        .expect("the right pair should sign in");
    let acting = |operator: bool| format!("{} operator: {operator}", ada.acting());

    // then: the work is told it acts as no operator
    assert_eq!(
        work.get("/acting").add_header(COOKIE, &cookie).await.text(),
        acting(false)
    );

    // when: the account becomes an operator
    let code = wired
        .accounts
        .draw_operator_code()
        .await
        .expect("drawing should succeed")
        .expect("with nobody appointed a code is drawn");
    wired
        .accounts
        .become_operator(&ada, code.as_str())
        .await
        .expect("the code drawn makes an operator");

    // then: the work is told it acts as an operator within the same session
    assert_eq!(
        work.get("/acting").add_header(COOKIE, &cookie).await.text(),
        acting(true),
        "the same session counts the role from the next request on, read from the account"
    );
}
