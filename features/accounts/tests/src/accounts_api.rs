use accounts_contract::{
    NewPasswordRequest, PasswordResetRequest, SendCodeRequest, SignUpRequest, VerifyEmailRequest,
    refused,
};
use axum::http::StatusCode;
use axum::http::header::{CONTENT_TYPE, RETRY_AFTER};
use axum_test::{TestResponse, TestServer};
use serde_json::Value;
use time::Duration;

use crate::wiring;
use crate::wiring::{ADA, HER_PASSWORD, Wired};

fn a_server() -> (Wired, TestServer) {
    let wired = wiring::wired();
    let server = TestServer::new(wired.routes.clone());

    (wired, server)
}

fn a_sign_up(email: &str, name: &str, password: &str) -> SignUpRequest {
    SignUpRequest {
        email: email.to_owned(),
        name: name.to_owned(),
        password: password.to_owned(),
        operator_code: None,
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
    // given: a server
    let (wired, server) = a_server();

    // when: an account signs up
    server
        .post("/accounts")
        .json(&a_sign_up(ADA, "Ada", HER_PASSWORD))
        .await
        .assert_status(StatusCode::ACCEPTED);

    // and: a code is sent
    assert_eq!(wired.codes_sent_to(ADA).len(), 1);
}

#[tokio::test]
async fn an_address_already_taken_is_answered_like_any_other() {
    // given: an account signed up
    let (_wired, server) = a_server();
    server
        .post("/accounts")
        .json(&a_sign_up(ADA, "Ada", HER_PASSWORD))
        .await
        .assert_status(StatusCode::ACCEPTED);

    // then: signing up again with its address is accepted
    server
        .post("/accounts")
        .json(&a_sign_up(ADA, "Mallory", "another long password"))
        .await
        .assert_status(StatusCode::ACCEPTED);
}

#[tokio::test]
async fn what_cannot_make_an_account_is_refused_by_its_kind() {
    // given: a server
    let (_wired, server) = a_server();

    // then: each sign up that cannot make an account is refused by its kind
    for (request, kind) in [
        (
            a_sign_up("ada", "Ada", HER_PASSWORD),
            refused::EMAIL_MALFORMED,
        ),
        (
            a_sign_up(
                &format!("{}@example.com", "a".repeat(300)),
                "Ada",
                HER_PASSWORD,
            ),
            refused::EMAIL_TOO_LONG,
        ),
        (a_sign_up(ADA, "  ", HER_PASSWORD), refused::NAME_BLANK),
        (
            a_sign_up(ADA, "A\u{7}da", HER_PASSWORD),
            refused::NAME_CONTROL_CHARACTER,
        ),
        (
            a_sign_up(ADA, &"a".repeat(101), HER_PASSWORD),
            refused::NAME_TOO_LONG,
        ),
        (a_sign_up(ADA, "Ada", "short"), refused::PASSWORD_TOO_SHORT),
        (
            a_sign_up(ADA, "Ada", &"a".repeat(257)),
            refused::PASSWORD_TOO_LONG,
        ),
    ] {
        refused_as(server.post("/accounts").json(&request).await, kind);
    }
}

#[tokio::test]
async fn a_refused_length_says_its_numbers() {
    // given: a server
    let (_wired, server) = a_server();

    // when: a sign up with too short a password is refused
    let told = refused_as(
        server
            .post("/accounts")
            .json(&a_sign_up(ADA, "Ada", "short"))
            .await,
        refused::PASSWORD_TOO_SHORT,
    );

    // then: the refusal says the least length and the length given
    assert_eq!(told["min"], 12);
    assert_eq!(told["actual"], 5);
}

#[tokio::test]
async fn the_code_sent_verifies_the_address() {
    // given: an account signed up
    let (wired, server) = a_server();
    server
        .post("/accounts")
        .json(&a_sign_up(ADA, "Ada", HER_PASSWORD))
        .await
        .assert_status(StatusCode::ACCEPTED);

    // then: verifying with the code sent succeeds
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
    // given: a server
    let (_wired, server) = a_server();

    // then: verifying with a wrong code is refused as wrong
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
    // given: an account signed up with a code sent
    let (wired, server) = a_server();
    server
        .post("/accounts")
        .json(&a_sign_up(ADA, "Ada", HER_PASSWORD))
        .await
        .assert_status(StatusCode::ACCEPTED);
    let code = wired.last_code_sent_to(ADA);

    // when: the code has expired
    wired
        .clock
        .set(wiring::at(1_700_000_000) + Duration::hours(1));

    // then: verifying with it is refused as expired
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

    // when: the code is good again but every try is spent on wrong guesses
    wired.clock.set(wiring::at(1_700_000_000));
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

    // then: verifying with it is refused as exhausted
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
    // given: an account signed up
    let (wired, server) = a_server();
    server
        .post("/accounts")
        .json(&a_sign_up(ADA, "Ada", HER_PASSWORD))
        .await
        .assert_status(StatusCode::ACCEPTED);

    // when: a code is asked for again for its address and for one nobody has
    for email in [ADA, "nobody@example.com"] {
        server
            .post("/accounts/verification/code")
            .json(&SendCodeRequest {
                email: email.to_owned(),
            })
            .await
            .assert_status(StatusCode::ACCEPTED);
    }

    // then: the account is sent a second code
    assert_eq!(wired.codes_sent_to(ADA).len(), 2);
}

#[tokio::test]
async fn every_refusal_kind_is_named_once() {
    // expect: no refusal kind is named twice
    let mut kinds = refused::EVERY.to_vec();
    kinds.sort_unstable();
    kinds.dedup();

    assert_eq!(kinds.len(), refused::EVERY.len());
}

#[tokio::test]
async fn a_password_is_reset_by_the_code_sent_for_it() {
    // given: an account signed up
    let (wired, server) = a_server();
    server
        .post("/accounts")
        .json(&a_sign_up(ADA, "Ada", HER_PASSWORD))
        .await
        .assert_status(StatusCode::ACCEPTED);

    // when: it asks to reset its password
    server
        .post("/accounts/password-reset")
        .json(&PasswordResetRequest {
            email: ADA.to_owned(),
        })
        .await
        .assert_status(StatusCode::ACCEPTED);
    // and: sets a new one with the code sent
    server
        .post("/accounts/password")
        .json(&NewPasswordRequest {
            email: ADA.to_owned(),
            code: wired.last_code_sent_to(ADA),
            password: "a better horse, a better battery".to_owned(),
        })
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: it signs in with the new one
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
    // given: a server
    let (_wired, server) = a_server();

    // then: a reset with a wrong code is refused as a wrong code
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
    // and: one with too short a password as too short
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
    // given: a server
    let (_wired, server) = a_server();

    // then: asking to reset for an address nobody has is accepted
    server
        .post("/accounts/password-reset")
        .json(&PasswordResetRequest {
            email: "nobody@example.com".to_owned(),
        })
        .await
        .assert_status(StatusCode::ACCEPTED);
}

#[tokio::test]
async fn too_many_codes_asked_for_are_answered_with_how_long_to_wait() {
    // given: as many resets asked for an address as are allowed
    let (_wired, server) = a_server();
    let asking = PasswordResetRequest {
        email: ADA.to_owned(),
    };
    for _ in 0..3 {
        server
            .post("/accounts/password-reset")
            .json(&asking)
            .await
            .assert_status(StatusCode::ACCEPTED);
    }

    // when: one more is asked for
    let response = server.post("/accounts/password-reset").json(&asking).await;

    // then: it is answered with too many requests
    response.assert_status(StatusCode::TOO_MANY_REQUESTS);
    // and: how long to wait
    assert_eq!(response.header(RETRY_AFTER), "60");
    // and: why
    assert_eq!(response.json::<Value>()["type"], refused::THROTTLED);
}
