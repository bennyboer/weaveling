use accounts_contract::{BecomeOperatorRequest, SESSION_COOKIE, SignInRequest, refused};
use axum::http::StatusCode;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum_test::TestServer;
use serde_json::Value;

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

async fn served_with_ada_signed_in() -> (Wired, TestServer, String) {
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
    let server = TestServer::new(wired.routes.clone().merge(
        accounts_rest::admitting_only_the_signed_in(
            wired.accounts.clone(),
            accounts_rest::guarded_router(wired.accounts.clone()),
        ),
    ));

    let signed_in = server
        .post("/sessions")
        .json(&SignInRequest {
            email: ADA.to_owned(),
            password: HER_PASSWORD.to_owned(),
        })
        .await;
    let cookie = signed_in
        .header(SET_COOKIE)
        .to_str()
        .expect("a cookie is plain text")
        .split(';')
        .next()
        .expect("the cookie names the session")
        .to_owned();
    assert!(cookie.starts_with(SESSION_COOKIE));

    (wired, server, cookie)
}

async fn drawn(wired: &Wired) -> String {
    wired
        .accounts
        .draw_operator_code()
        .await
        .expect("drawing should succeed")
        .expect("with nobody appointed a code is drawn")
        .as_str()
        .to_owned()
}

async fn is_operator(server: &TestServer, cookie: &str) -> bool {
    server
        .get("/sessions/current")
        .add_header(COOKIE, cookie)
        .await
        .json::<Value>()["operator"]
        .as_bool()
        .expect("who is signed in says whether they are an operator")
}

#[tokio::test]
async fn the_code_makes_whoever_is_signed_in_an_operator() {
    let (wired, server, cookie) = served_with_ada_signed_in().await;
    let code = drawn(&wired).await;
    assert!(!is_operator(&server, &cookie).await);

    server
        .post("/operators")
        .add_header(COOKIE, &cookie)
        .json(&BecomeOperatorRequest { code })
        .await
        .assert_status(StatusCode::NO_CONTENT);

    assert!(is_operator(&server, &cookie).await);
}

#[tokio::test]
async fn a_wrong_code_is_refused_as_such() {
    let (wired, server, cookie) = served_with_ada_signed_in().await;
    let _code = drawn(&wired).await;

    let refused = server
        .post("/operators")
        .add_header(COOKIE, &cookie)
        .json(&BecomeOperatorRequest {
            code: "not the code at all".to_owned(),
        })
        .await;

    refused.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(
        refused.json::<Value>()["type"],
        refused::OPERATOR_CODE_WRONG
    );
    assert!(!is_operator(&server, &cookie).await);
}

#[tokio::test]
async fn nobody_becomes_an_operator_without_signing_in() {
    let (wired, server, _cookie) = served_with_ada_signed_in().await;
    let code = drawn(&wired).await;

    let refused = server
        .post("/operators")
        .json(&BecomeOperatorRequest { code })
        .await;

    refused.assert_status(StatusCode::UNAUTHORIZED);
    assert_eq!(refused.json::<Value>()["type"], refused::NOT_SIGNED_IN);
}
