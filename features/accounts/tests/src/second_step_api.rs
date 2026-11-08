use accounts_contract::{RecoveryCodesDTO, refused, stage};
use axum::http::StatusCode;

use crate::totp_bench::{
    code_of, cookie_set_by, refused_as, with_a_key, with_a_second_factor_set_up_at,
};
use crate::wiring::HER_PASSWORD;

const SET_UP_AT: i64 = 1_700_000_060;
const A_STEP_LATER: i64 = SET_UP_AT + 30;

#[tokio::test]
async fn a_password_alone_opens_a_session_that_may_only_answer_the_second_factor() {
    // given: Ada with a second factor
    let set_up = with_a_second_factor_set_up_at(SET_UP_AT).await;
    let served = &set_up.served;

    // when: she signs in with her password
    let owing = served.signed_in().await;

    // then: her session owes its second factor
    assert_eq!(served.stage_of(&owing).await, stage::SECOND_FACTOR_OWED);
    // and: it may do nothing else
    refused_as(
        &served.begin(&owing).await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
}

#[tokio::test]
async fn a_code_of_the_app_signs_in_all_the_way_under_a_new_cookie() {
    // given: Ada signed in with her password, owing her second factor
    let set_up = with_a_second_factor_set_up_at(SET_UP_AT).await;
    let served = &set_up.served;
    let owing = served.signed_in().await;

    // when: she enters the code her app shows a step later
    let now = served.at(A_STEP_LATER);
    let answered = served.answer(&owing, &code_of(&set_up.secret, now)).await;

    // then: she is signed in all the way, under a new cookie
    answered.assert_status_ok();
    let signed_in = cookie_set_by(&answered);
    assert_eq!(served.stage_of(&signed_in).await, stage::SIGNED_IN);
    // and: the cookie that owed it signs in no more
    refused_as(
        &served.current(&owing).await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
}

#[tokio::test]
async fn a_code_serves_once_not_even_the_one_that_finished_setting_up_again() {
    // given: Ada signed in twice with her password, in the step her second factor was set up in
    let set_up = with_a_second_factor_set_up_at(SET_UP_AT).await;
    let served = &set_up.served;
    let one = served.signed_in().await;
    let other = served.signed_in().await;

    // when: she answers with the code that finished setting up
    let again = served
        .answer(&one, &code_of(&set_up.secret, SET_UP_AT))
        .await;

    // then: it is refused
    refused_as(
        &again,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::TOTP_CODE_WRONG,
    );
    // when: a step later one session answers, and the other with the same code
    let code = code_of(&set_up.secret, served.at(A_STEP_LATER));
    served.answer(&one, &code).await.assert_status_ok();
    let replayed = served.answer(&other, &code).await;
    // then: the second is refused
    refused_as(
        &replayed,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::TOTP_CODE_WRONG,
    );
}

#[tokio::test]
async fn a_recovery_code_signs_in_once_and_leaves_one_fewer() {
    // given: Ada signed in with her password, owing her second factor
    let set_up = with_a_second_factor_set_up_at(SET_UP_AT).await;
    let served = &set_up.served;
    let owing = served.signed_in().await;

    // when: she answers with a recovery code, typed in capitals without the dash
    let typed = set_up.recovery_codes[0].replace('-', "").to_uppercase();
    let answered = served.answer_with_recovery_code(&owing, &typed).await;

    // then: she is signed in all the way
    answered.assert_status_ok();
    let signed_in = cookie_set_by(&answered);
    assert_eq!(served.stage_of(&signed_in).await, stage::SIGNED_IN);
    // and: nine codes are left
    assert_eq!(
        served
            .second_factor_shown(&signed_in)
            .await
            .map(|factor| factor.recovery_codes_left),
        Some(9)
    );
    // when: she signs in again and answers with the same code
    let owing = served.signed_in().await;
    let again = served
        .answer_with_recovery_code(&owing, &set_up.recovery_codes[0])
        .await;
    // then: it is refused
    refused_as(
        &again,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::RECOVERY_CODE_WRONG,
    );
}

#[tokio::test]
async fn nothing_is_answered_where_no_second_factor_is_owed() {
    // given: Ada with a second factor, signed in all the way
    let set_up = with_a_second_factor_set_up_at(SET_UP_AT).await;
    let served = &set_up.served;
    let code = code_of(&set_up.secret, served.at(A_STEP_LATER));

    // expect: her signed-in session answers nothing
    refused_as(
        &served.answer(&set_up.cookie, &code).await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
    // and: neither does a request without a session
    refused_as(
        &served.answer("weaveling_session=nobody", &code).await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
}

#[tokio::test]
async fn a_session_owing_its_second_factor_runs_out_after_ten_minutes() {
    // given: Ada signed in with her password
    let set_up = with_a_second_factor_set_up_at(SET_UP_AT).await;
    let served = &set_up.served;
    let owing = served.signed_in().await;

    // when: she answers eleven minutes later
    let now = served.at(SET_UP_AT + 11 * 60);
    let answered = served.answer(&owing, &code_of(&set_up.secret, now)).await;

    // then: she has to sign in again
    refused_as(&answered, StatusCode::UNAUTHORIZED, refused::NOT_SIGNED_IN);
}

#[tokio::test]
async fn wrong_answers_of_either_kind_are_tried_five_times_before_waiting() {
    // given: Ada signed in with her password
    let set_up = with_a_second_factor_set_up_at(SET_UP_AT).await;
    let served = &set_up.served;
    let owing = served.signed_in().await;

    // when: she answers wrongly five times, with codes and recovery codes in turn
    for tried in 0..5 {
        let wrong = if tried % 2 == 0 {
            served.answer(&owing, "000000").await
        } else {
            served
                .answer_with_recovery_code(&owing, "aaaaa-aaaaa")
                .await
        };
        wrong.assert_status(StatusCode::UNPROCESSABLE_ENTITY);
    }

    // then: the sixth try is told to wait, even with the right code
    let now = served.at(A_STEP_LATER);
    refused_as(
        &served.answer(&owing, &code_of(&set_up.secret, now)).await,
        StatusCode::TOO_MANY_REQUESTS,
        refused::THROTTLED,
    );
}

#[tokio::test]
async fn new_recovery_codes_are_drawn_for_the_password_and_the_old_ones_end() {
    // given: Ada with a second factor, signed in all the way
    let set_up = with_a_second_factor_set_up_at(SET_UP_AT).await;
    let served = &set_up.served;

    // when: she draws new recovery codes with her password
    let drawn = served
        .draw_recovery_codes(&set_up.cookie, HER_PASSWORD)
        .await;

    // then: ten new codes are shown
    drawn.assert_status_ok();
    let new: RecoveryCodesDTO = drawn.json();
    assert_eq!(new.codes.len(), 10);
    assert!(
        new.codes
            .iter()
            .all(|code| !set_up.recovery_codes.contains(code))
    );
    // and: an old one signs in no more
    let owing = served.signed_in().await;
    refused_as(
        &served
            .answer_with_recovery_code(&owing, &set_up.recovery_codes[0])
            .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::RECOVERY_CODE_WRONG,
    );
    // and: a new one does
    served
        .answer_with_recovery_code(&owing, &new.codes[0])
        .await
        .assert_status_ok();
}

#[tokio::test]
async fn drawing_new_recovery_codes_takes_the_password_and_a_second_factor() {
    // given: Ada with a second factor, signed in all the way
    let set_up = with_a_second_factor_set_up_at(SET_UP_AT).await;
    let served = &set_up.served;

    // expect: a wrong password draws nothing
    refused_as(
        &served
            .draw_recovery_codes(&set_up.cookie, "not her password")
            .await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::PASSWORD_WRONG,
    );
    // and: an account without a second factor has none to draw for
    let without = with_a_key().await;
    let cookie = without.signed_in().await;
    refused_as(
        &without.draw_recovery_codes(&cookie, HER_PASSWORD).await,
        StatusCode::CONFLICT,
        refused::NO_SECOND_FACTOR,
    );
}
