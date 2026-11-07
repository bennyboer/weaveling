use accounts_contract::{RecoveryCodesDTO, refused};
use accounts_core::SecondFactors;
use axum::http::StatusCode;

use crate::totp_bench::{code_of, cookie_set_by, refused_as, served, with_a_key};

#[tokio::test]
async fn without_a_key_no_second_factor_is_set_up() {
    // given: an instance without a key, and Ada signed in
    let served = served(|wired| wired.accounts.clone()).await;
    let cookie = served.signed_in().await;

    // when: she begins setting up a second factor
    let begun = served.begin(&cookie).await;

    // then: it is refused, as the instance cannot keep one
    refused_as(&begun, StatusCode::CONFLICT, refused::TOTP_UNAVAILABLE);
}

#[tokio::test]
async fn setting_up_shows_a_secret_to_type_and_the_address_its_qr_code_holds() {
    // given: Ada signed in
    let served = with_a_key().await;
    let cookie = served.signed_in().await;

    // when: she begins setting up a second factor
    let setting_up = served.begun(&cookie).await;

    // then: she is shown a secret to type, of 32 characters
    assert_eq!(setting_up.secret.len(), 32);
    // and: the address for her app, naming Weaveling and her
    assert_eq!(
        setting_up.otpauth_uri,
        format!(
            "otpauth://totp/Weaveling:ada%40example.com?secret={}&issuer=Weaveling",
            setting_up.secret
        )
    );
    // and: she has no second factor yet
    assert_eq!(served.second_factor_shown(&cookie).await, None);
}

#[tokio::test]
async fn a_code_of_the_app_finishes_setting_up_and_ten_recovery_codes_are_shown_once() {
    // given: Ada setting up a second factor
    let served = with_a_key().await;
    let cookie = served.signed_in().await;
    let setting_up = served.begun(&cookie).await;

    // when: she enters the code her app shows a minute later
    let now = served.at(1_700_000_060);
    let finished = served
        .finish(&cookie, &code_of(&setting_up.secret, now))
        .await;

    // then: she is shown ten recovery codes, each two halves of five
    finished.assert_status_ok();
    let shown: RecoveryCodesDTO = finished.json();
    assert_eq!(shown.codes.len(), 10);
    assert!(
        shown
            .codes
            .iter()
            .all(|code| code.len() == 11 && &code[5..6] == "-")
    );
    // and: the codes are kept hashed, none as shown
    let kept = served
        .wired
        .secrets
        .recovery_codes_of(&served.ada)
        .await
        .expect("reading should succeed");
    assert_eq!(kept.len(), 10);
    assert!(kept.iter().all(|hash| {
        shown
            .codes
            .iter()
            .all(|code| !hash.as_str().contains(&code[..5]))
    }));
    // and: her session goes on under a new cookie, with the second factor in use since then
    let renewed = cookie_set_by(&finished);
    let in_use = served
        .second_factor_shown(&renewed)
        .await
        .expect("a second factor is in use");
    assert_eq!(in_use.recovery_codes_left, 10);
    assert_eq!(in_use.since, "2023-11-14T22:14:20Z");
}

#[tokio::test]
async fn finishing_ends_every_other_session_and_the_old_cookie_of_this_one() {
    // given: Ada signed in on two devices
    let served = with_a_key().await;
    let here = served.signed_in().await;
    let elsewhere = served.signed_in().await;
    // and: setting up a second factor on one of them
    let setting_up = served.begun(&here).await;

    // when: she finishes setting it up a minute later
    let now = served.at(1_700_000_060);
    let finished = served
        .finish(&here, &code_of(&setting_up.secret, now))
        .await;
    finished.assert_status_ok();

    // then: the other device is signed out
    refused_as(
        &served.current(&elsewhere).await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
    // and: the cookie she finished with no longer signs in
    refused_as(
        &served.current(&here).await,
        StatusCode::UNAUTHORIZED,
        refused::NOT_SIGNED_IN,
    );
    // and: the new cookie does
    served
        .current(&cookie_set_by(&finished))
        .await
        .assert_status_ok();
}

#[tokio::test]
async fn a_wrong_code_finishes_nothing_and_leaves_the_session_as_it_was() {
    // given: Ada setting up a second factor
    let served = with_a_key().await;
    let cookie = served.signed_in().await;
    let setting_up = served.begun(&cookie).await;

    // when: she enters a code of another moment
    let finished = served
        .finish(&cookie, &code_of(&setting_up.secret, 1_600_000_000))
        .await;

    // then: it is refused
    refused_as(
        &finished,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::TOTP_CODE_WRONG,
    );
    // and: her session goes on, without a second factor
    assert_eq!(served.second_factor_shown(&cookie).await, None);
}

#[tokio::test]
async fn finishing_what_was_never_begun_or_already_finished_is_refused() {
    // given: Ada signed in, setting nothing up
    let served = with_a_key().await;
    let cookie = served.signed_in().await;

    // when: she finishes setting up
    let finished = served.finish(&cookie, "123456").await;

    // then: it is refused, as nothing is being set up
    refused_as(
        &finished,
        StatusCode::CONFLICT,
        refused::NOTHING_BEING_SET_UP,
    );
    // when: she sets one up and finishes it
    let setting_up = served.begun(&cookie).await;
    let now = served.at(1_700_000_060);
    let finished = served
        .finish(&cookie, &code_of(&setting_up.secret, now))
        .await;
    finished.assert_status_ok();
    // and: finishes again a step later
    let now = served.at(1_700_000_090);
    let again = served
        .finish(&cookie_set_by(&finished), &code_of(&setting_up.secret, now))
        .await;
    // then: it is refused, as nothing is being set up any more
    refused_as(&again, StatusCode::CONFLICT, refused::NOTHING_BEING_SET_UP);
}

#[tokio::test]
async fn wrong_codes_are_tried_five_times_before_waiting() {
    // given: Ada setting up a second factor
    let served = with_a_key().await;
    let cookie = served.signed_in().await;
    let setting_up = served.begun(&cookie).await;
    let wrong = code_of(&setting_up.secret, 1_600_000_000);

    // when: she enters a wrong code five times
    for _ in 0..5 {
        refused_as(
            &served.finish(&cookie, &wrong).await,
            StatusCode::UNPROCESSABLE_ENTITY,
            refused::TOTP_CODE_WRONG,
        );
    }

    // then: the sixth try is told to wait, even with the right code
    let right = code_of(&setting_up.secret, 1_700_000_000);
    refused_as(
        &served.finish(&cookie, &right).await,
        StatusCode::TOO_MANY_REQUESTS,
        refused::THROTTLED,
    );
}

#[tokio::test]
async fn a_second_factor_set_up_anew_stays_as_it_was_until_the_new_one_is_finished() {
    // given: Ada with a second factor in use
    let served = with_a_key().await;
    let cookie = served.signed_in().await;
    let first = served.begun(&cookie).await;
    let now = served.at(1_700_000_060);
    let finished = served.finish(&cookie, &code_of(&first.secret, now)).await;
    let cookie = cookie_set_by(&finished);

    // when: she begins setting up another, an hour later
    served.at(1_700_003_600);
    let anew = served.begun(&cookie).await;

    // then: a new secret is drawn
    assert_ne!(anew.secret, first.secret);
    // and: the first stays in use, as it was
    assert_eq!(
        served
            .second_factor_shown(&cookie)
            .await
            .map(|factor| factor.since),
        Some("2023-11-14T22:14:20Z".to_owned())
    );
    // and: a code of the first does not finish the new one
    let now = served.at(1_700_003_630);
    refused_as(
        &served.finish(&cookie, &code_of(&first.secret, now)).await,
        StatusCode::UNPROCESSABLE_ENTITY,
        refused::TOTP_CODE_WRONG,
    );
}

#[tokio::test]
async fn finishing_a_second_factor_set_up_anew_goes_on_signed_in_all_the_way() {
    // given: Ada with a second factor in use, setting up another
    let set_up = crate::totp_bench::with_a_second_factor_set_up_at(1_700_000_060).await;
    let served = &set_up.served;
    let anew = served.begun(&set_up.cookie).await;

    // when: she finishes it with a code of the new one
    let now = served.at(1_700_000_120);
    let finished = served
        .finish(&set_up.cookie, &code_of(&anew.secret, now))
        .await;

    // then: her session goes on signed in all the way, not owing a second factor
    finished.assert_status_ok();
    assert_eq!(
        served.stage_of(&cookie_set_by(&finished)).await,
        accounts_contract::stage::SIGNED_IN,
        "the session that just proved the new factor owes none"
    );
}
