use accounts_contract::{AccountPageDTO, PasswordRequest, SignInRequest, refused, stage};
use accounts_core::{AccountEvent, AccountId, EmailAddress, EmailAddresses, KIND, SecondFactors};
use axum::http::StatusCode;
use axum::http::header::{COOKIE, SET_COOKIE};
use axum_test::TestResponse;
use eventsourcing::{AggregateId, EventStore, Version};

use crate::totp_bench::{
    Served, code_of, cookie_set_by, refused_as, with_a_key, with_a_second_factor_set_up_at,
};
use crate::wiring::HER_PASSWORD;

const SET_UP_AT: i64 = 1_700_000_060;
const A_STEP_LATER: i64 = SET_UP_AT + 30;
const GRACE: &str = "grace@example.com";

impl Served {
    async fn turn_off(&self, cookie: &str, password: &str) -> TestResponse {
        self.server
            .delete("/accounts/current/totp")
            .add_header(COOKIE, cookie.to_owned())
            .json(&PasswordRequest {
                password: password.to_owned(),
            })
            .await
    }

    async fn removed_by(&self, cookie: &str, account: &AccountId) -> TestResponse {
        self.server
            .delete(&format!("/accounts/{account}/totp"))
            .add_header(COOKIE, cookie.to_owned())
            .await
    }

    async fn listed_with_a_second_factor(&self, cookie: &str, account: &AccountId) -> bool {
        self.wired.settle().await;
        let listed: AccountPageDTO = self
            .server
            .get("/accounts")
            .add_header(COOKIE, cookie.to_owned())
            .await
            .json();

        listed
            .accounts
            .iter()
            .find(|summary| summary.id == account.to_string())
            .expect("the account is listed")
            .second_factor
    }

    pub(crate) async fn grace_the_operator(&self) -> (AccountId, String) {
        let accounts = &self.wired.accounts;
        accounts
            .sign_up(GRACE, "Grace", HER_PASSWORD)
            .await
            .expect("signing up should succeed");
        accounts
            .verify(GRACE, &self.wired.last_code_sent_to(GRACE))
            .await
            .expect("the code sent should verify");
        let grace = self
            .wired
            .secrets
            .holder(&EmailAddress::new(GRACE).expect("a plain address"))
            .await
            .expect("reading should succeed")
            .expect("the address should be held");
        let code = accounts
            .draw_operator_code()
            .await
            .expect("drawing should succeed")
            .expect("with nobody appointed a code is drawn");
        accounts
            .become_operator(&grace, code.as_str())
            .await
            .expect("the code drawn makes an operator");

        let cookie = cookie_set_by(
            &self
                .server
                .post("/sessions")
                .json(&SignInRequest {
                    email: GRACE.to_owned(),
                    password: HER_PASSWORD.to_owned(),
                })
                .await,
        );

        (grace, cookie)
    }
}

#[tokio::test]
async fn her_password_turns_totp_off_and_a_password_signs_in_alone_again() {
    // given: Ada with a second factor, signed in on two devices
    let set_up = with_a_second_factor_set_up_at(SET_UP_AT).await;
    let served = &set_up.served;
    let now = served.at(A_STEP_LATER);
    let elsewhere = served.signed_in().await;
    let elsewhere = cookie_set_by(
        &served
            .answer(&elsewhere, &code_of(&set_up.secret, now))
            .await,
    );

    // when: she turns it off on one with her password, a step later
    served.at(A_STEP_LATER + 30);
    let turned_off = served.turn_off(&set_up.cookie, HER_PASSWORD).await;

    // then: it is off, and her session goes on under a new cookie
    turned_off.assert_status(StatusCode::NO_CONTENT);
    let renewed = cookie_set_by(&turned_off);
    assert_eq!(served.second_factor_shown(&renewed).await, None);
    // and: the other device is signed out
    refused_as(
        &served.current(&elsewhere).await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
    // and: her secret and recovery codes are forgotten, nothing else answering the second step
    let secrets = &served.wired.secrets;
    assert_eq!(
        secrets
            .totp_of(&served.ada)
            .await
            .expect("reading should succeed"),
        None
    );
    assert!(
        secrets
            .recovery_codes_of(&served.ada)
            .await
            .expect("reading should succeed")
            .is_empty()
    );
    // and: her password alone signs her in again
    let signed_in = served.signed_in().await;
    assert_eq!(served.stage_of(&signed_in).await, stage::SIGNED_IN);
}

#[tokio::test]
async fn a_wrong_password_turns_nothing_off() {
    // given: Ada with a second factor, signed in
    let set_up = with_a_second_factor_set_up_at(SET_UP_AT).await;
    let served = &set_up.served;

    // when: she turns it off with a password that is not hers
    let refused = served.turn_off(&set_up.cookie, "not her password").await;

    // then: it is refused, and the second factor stays
    refused_as(
        &refused,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::PASSWORD_WRONG,
    );
    assert!(served.second_factor_shown(&set_up.cookie).await.is_some());
    // and: the cookie she has still signs her in
    assert!(refused.maybe_header(SET_COOKIE).is_none());
}

#[tokio::test]
async fn nothing_is_turned_off_where_there_is_no_second_factor() {
    // given: Ada without a second factor, signed in
    let served = with_a_key().await;
    let cookie = served.signed_in().await;

    // expect: turning it off is refused
    refused_as(
        &served.turn_off(&cookie, HER_PASSWORD).await,
        StatusCode::CONFLICT,
        refused::NO_SECOND_FACTOR,
    );
}

#[tokio::test]
async fn the_operator_turns_a_second_factor_off_and_its_sessions_end() {
    // given: Ada with a second factor, signed in, and Grace the operator
    let set_up = with_a_second_factor_set_up_at(SET_UP_AT).await;
    let served = &set_up.served;
    let (grace, cookie) = served.grace_the_operator().await;
    // and: Grace sees in the list of accounts that Ada has one
    assert!(
        served
            .listed_with_a_second_factor(&cookie, &served.ada)
            .await
    );

    // when: Grace turns Ada's second factor off
    served.at(A_STEP_LATER);
    let removed = served.removed_by(&cookie, &served.ada).await;

    // then: it is off, in the list of accounts too
    removed.assert_status(StatusCode::NO_CONTENT);
    assert!(
        !served
            .listed_with_a_second_factor(&cookie, &served.ada)
            .await
    );
    assert_eq!(
        served
            .wired
            .secrets
            .totp_of(&served.ada)
            .await
            .expect("reading should succeed"),
        None
    );
    // and: Ada is signed out, and signs in by her password alone
    refused_as(
        &served.current(&set_up.cookie).await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
    let signed_in = served.signed_in().await;
    assert_eq!(served.stage_of(&signed_in).await, stage::SIGNED_IN);
    // and: her history says Grace did it
    let told = served
        .wired
        .store
        .read_from(&AggregateId::from(&served.ada), KIND, Version::of(1))
        .await
        .expect("reading should succeed");
    let removed = told
        .iter()
        .find(|recorded| matches!(recorded.event, AccountEvent::TotpRemoved))
        .expect("the removal should be told");
    assert_eq!(removed.metadata.agent, grace.acting());
}

#[tokio::test]
async fn only_an_operator_turns_another_accounts_second_factor_off() {
    // given: Ada with a second factor, and Grace who is no operator
    let set_up = with_a_second_factor_set_up_at(SET_UP_AT).await;
    let served = &set_up.served;
    let accounts = &served.wired.accounts;
    accounts
        .sign_up(GRACE, "Grace", HER_PASSWORD)
        .await
        .expect("signing up should succeed");
    accounts
        .verify(GRACE, &served.wired.last_code_sent_to(GRACE))
        .await
        .expect("the code sent should verify");
    let grace = cookie_set_by(
        &served
            .server
            .post("/sessions")
            .json(&SignInRequest {
                email: GRACE.to_owned(),
                password: HER_PASSWORD.to_owned(),
            })
            .await,
    );

    // when: Grace turns Ada's second factor off
    let refused = served.removed_by(&grace, &served.ada).await;

    // then: it is refused, and Ada keeps it
    refused_as(&refused, StatusCode::FORBIDDEN, refused::NOT_AN_OPERATOR);
    assert!(
        served
            .wired
            .secrets
            .totp_of(&served.ada)
            .await
            .expect("reading should succeed")
            .is_some()
    );
}
