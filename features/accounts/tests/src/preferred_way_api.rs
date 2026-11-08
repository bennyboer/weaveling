use accounts_contract::{PreferredWayRequest, refused, second_step_way};
use axum::http::StatusCode;
use axum::http::header::COOKIE;
use axum_test::TestResponse;

use crate::totp_bench::{Served, refused_as, with_a_second_factor_set_up_at};

const SET_UP_AT: i64 = 1_700_000_060;

impl Served {
    async fn choose_first(&self, cookie: &str, preferred_way: &str) -> TestResponse {
        self.server
            .put("/accounts/current/second-step/preferred")
            .add_header(COOKIE, cookie.to_owned())
            .json(&PreferredWayRequest {
                preferred_way: preferred_way.to_owned(),
            })
            .await
    }
}

#[tokio::test]
async fn a_session_tells_which_way_the_second_step_is_asked_first() {
    // given: Ada with TOTP, never having chosen
    let set_up = with_a_second_factor_set_up_at(SET_UP_AT).await;
    let served = &set_up.served;

    // when: her session is asked
    let factor = served
        .second_factor_shown(&set_up.cookie)
        .await
        .expect("a second factor");

    // then: it is asked as it was answered last, which is the app, the only way
    assert_eq!(factor.preferred_way, second_step_way::LAST_USED);
    assert_eq!(factor.asked_way, second_step_way::TOTP);
}

#[tokio::test]
async fn a_way_chosen_is_told_and_one_that_answers_nothing_gives_way() {
    // given: Ada with TOTP alone
    let set_up = with_a_second_factor_set_up_at(SET_UP_AT).await;
    let served = &set_up.served;

    // when: she chooses keys first
    served
        .choose_first(&set_up.cookie, second_step_way::KEY)
        .await
        .assert_status(StatusCode::NO_CONTENT);

    // then: the choice is told, while the app is still asked first, no key answering
    let factor = served
        .second_factor_shown(&set_up.cookie)
        .await
        .expect("a second factor");
    assert_eq!(factor.preferred_way, second_step_way::KEY);
    assert_eq!(factor.asked_way, second_step_way::TOTP);
    // and: a way that is none is rejected
    refused_as(
        &served.choose_first(&set_up.cookie, "carrier-pigeon").await,
        StatusCode::BAD_REQUEST,
        refused::PREFERRED_WAY_UNKNOWN,
    );
}
