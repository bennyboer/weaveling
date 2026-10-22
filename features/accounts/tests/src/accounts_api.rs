use accounts_contract::{
    NewPasswordRequest, PasswordResetRequest, SendCodeRequest, SignUpRequest, VerifyEmailRequest,
    refused,
};
use axum::http::StatusCode;
use axum::http::header::CONTENT_TYPE;
use axum_test::{TestResponse, TestServer};
use serde_json::Value;
use time::Duration;

use crate::wiring::{ADA, HER_PASSWORD, Wired, at, wired};

fn served() -> (Wired, TestServer) {
    let wired = wired();
    let server = TestServer::new(wired.routes.clone());

    (wired, server)
}

fn signing_up(email: &str, name: &str, password: &str) -> SignUpRequest {
    SignUpRequest {
        email: email.to_owned(),
        name: name.to_owned(),
        password: password.to_owned(),
    }
}

fn refused_as(response: TestResponse, kind: &str) -> Value {
    response.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    assert_eq!(response.header(CONTENT_TYPE), "application/problem+json");
    let told: Value = response.json();
    assert_eq!(told["type"], kind, "{told}");

    told
}

#[tokio::test]
async fn signing_up_is_accepted_and_a_code_goes_out() {
    let (wired, server) = served();

    server
        .post("/accounts")
        .json(&signing_up(ADA, "Ada", HER_PASSWORD))
        .await
        .assert_status(StatusCode::ACCEPTED);

    assert_eq!(wired.codes_sent_to(ADA).len(), 1);
}

#[tokio::test]
async fn an_address_already_taken_is_answered_like_any_other() {
    let (_wired, server) = served();
    server
        .post("/accounts")
        .json(&signing_up(ADA, "Ada", HER_PASSWORD))
        .await
        .assert_status(StatusCode::ACCEPTED);

    server
        .post("/accounts")
        .json(&signing_up(ADA, "Mallory", "another long password"))
        .await
        .assert_status(StatusCode::ACCEPTED);
}

#[tokio::test]
async fn what_cannot_make_an_account_is_refused_by_its_kind() {
    let (_wired, server) = served();

    for (request, kind) in [
        (
            signing_up("ada", "Ada", HER_PASSWORD),
            refused::EMAIL_MALFORMED,
        ),
        (
            signing_up(
                &format!("{}@example.com", "a".repeat(300)),
                "Ada",
                HER_PASSWORD,
            ),
            refused::EMAIL_TOO_LONG,
        ),
        (signing_up(ADA, "  ", HER_PASSWORD), refused::NAME_BLANK),
        (
            signing_up(ADA, "A\u{7}da", HER_PASSWORD),
            refused::NAME_CONTROL_CHARACTER,
        ),
        (
            signing_up(ADA, &"a".repeat(101), HER_PASSWORD),
            refused::NAME_TOO_LONG,
        ),
        (signing_up(ADA, "Ada", "short"), refused::PASSWORD_TOO_SHORT),
        (
            signing_up(ADA, "Ada", &"a".repeat(257)),
            refused::PASSWORD_TOO_LONG,
        ),
    ] {
        refused_as(server.post("/accounts").json(&request).await, kind);
    }
}

#[tokio::test]
async fn a_refused_length_says_its_numbers() {
    let (_wired, server) = served();

    let told = refused_as(
        server
            .post("/accounts")
            .json(&signing_up(ADA, "Ada", "short"))
            .await,
        refused::PASSWORD_TOO_SHORT,
    );

    assert_eq!(told["min"], 12);
    assert_eq!(told["actual"], 5);
}

#[tokio::test]
async fn the_code_sent_verifies_the_address() {
    let (wired, server) = served();
    server
        .post("/accounts")
        .json(&signing_up(ADA, "Ada", HER_PASSWORD))
        .await
        .assert_status(StatusCode::ACCEPTED);

    server
        .post("/accounts/verification")
        .json(&VerifyEmailRequest {
            email: ADA.to_owned(),
            code: wired.last_code_sent_to(ADA),
        })
        .await
        .assert_status(StatusCode::NO_CONTENT);
}

#[tokio::test]
async fn a_wrong_code_is_refused_as_wrong() {
    let (_wired, server) = served();

    refused_as(
        server
            .post("/accounts/verification")
            .json(&VerifyEmailRequest {
                email: ADA.to_owned(),
                code: "123456".to_owned(),
            })
            .await,
        refused::CODE_WRONG,
    );
}

#[tokio::test]
async fn an_expired_or_exhausted_code_is_refused_as_such() {
    let (wired, server) = served();
    server
        .post("/accounts")
        .json(&signing_up(ADA, "Ada", HER_PASSWORD))
        .await
        .assert_status(StatusCode::ACCEPTED);
    let code = wired.last_code_sent_to(ADA);

    wired.clock.set(at(1_700_000_000) + Duration::hours(1));
    refused_as(
        server
            .post("/accounts/verification")
            .json(&VerifyEmailRequest {
                email: ADA.to_owned(),
                code: code.clone(),
            })
            .await,
        refused::CODE_EXPIRED,
    );

    wired.clock.set(at(1_700_000_000));
    let wrong = if code == "000000" { "000001" } else { "000000" };
    for _ in 0..5 {
        let _refused = server
            .post("/accounts/verification")
            .json(&VerifyEmailRequest {
                email: ADA.to_owned(),
                code: wrong.to_owned(),
            })
            .await;
    }
    refused_as(
        server
            .post("/accounts/verification")
            .json(&VerifyEmailRequest {
                email: ADA.to_owned(),
                code,
            })
            .await,
        refused::CODE_EXHAUSTED,
    );
}

#[tokio::test]
async fn asking_for_a_code_again_is_accepted_whoever_asks() {
    let (wired, server) = served();
    server
        .post("/accounts")
        .json(&signing_up(ADA, "Ada", HER_PASSWORD))
        .await
        .assert_status(StatusCode::ACCEPTED);

    for email in [ADA, "nobody@example.com"] {
        server
            .post("/accounts/verification/code")
            .json(&SendCodeRequest {
                email: email.to_owned(),
            })
            .await
            .assert_status(StatusCode::ACCEPTED);
    }

    assert_eq!(wired.codes_sent_to(ADA).len(), 2);
}

#[tokio::test]
async fn every_refusal_kind_is_named_once() {
    let mut kinds = refused::EVERY.to_vec();
    kinds.sort_unstable();
    kinds.dedup();

    assert_eq!(kinds.len(), refused::EVERY.len());
}

#[tokio::test]
async fn a_password_is_reset_by_the_code_sent_for_it() {
    let (wired, server) = served();
    server
        .post("/accounts")
        .json(&signing_up(ADA, "Ada", HER_PASSWORD))
        .await
        .assert_status(StatusCode::ACCEPTED);

    server
        .post("/accounts/password-reset")
        .json(&PasswordResetRequest {
            email: ADA.to_owned(),
        })
        .await
        .assert_status(StatusCode::ACCEPTED);
    server
        .post("/accounts/password")
        .json(&NewPasswordRequest {
            email: ADA.to_owned(),
            code: wired.last_code_sent_to(ADA),
            password: "a better horse, a better battery".to_owned(),
        })
        .await
        .assert_status(StatusCode::NO_CONTENT);

    assert!(
        wired
            .accounts
            .sign_in(ADA, "a better horse, a better battery")
            .await
            .is_ok()
    );
}

#[tokio::test]
async fn a_reset_is_refused_by_the_code_or_the_password_like_any_other() {
    let (_wired, server) = served();

    refused_as(
        server
            .post("/accounts/password")
            .json(&NewPasswordRequest {
                email: ADA.to_owned(),
                code: "123456".to_owned(),
                password: "a better horse, a better battery".to_owned(),
            })
            .await,
        refused::CODE_WRONG,
    );
    refused_as(
        server
            .post("/accounts/password")
            .json(&NewPasswordRequest {
                email: ADA.to_owned(),
                code: "123456".to_owned(),
                password: "short".to_owned(),
            })
            .await,
        refused::PASSWORD_TOO_SHORT,
    );
}

#[tokio::test]
async fn asking_to_reset_is_accepted_whoever_asks() {
    let (_wired, server) = served();

    server
        .post("/accounts/password-reset")
        .json(&PasswordResetRequest {
            email: "nobody@example.com".to_owned(),
        })
        .await
        .assert_status(StatusCode::ACCEPTED);
}
