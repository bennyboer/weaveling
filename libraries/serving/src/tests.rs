use axum::body;
use axum::http::header::{CONTENT_TYPE, IF_MATCH, RETRY_AFTER};
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use axum::response::IntoResponse;
use eventsourcing::{AggregateId, AggregateType, ServiceError, StoreError, Version};
use serde_json::{Value, json};
use thiserror::Error;
use time::Duration;

use super::*;

const KIND: AggregateType = AggregateType::of("idea");

#[derive(Debug, Error)]
#[error("an idea cannot be captured twice")]
struct AlreadyCaptured;

fn an_if_match(with: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        IF_MATCH,
        HeaderValue::from_str(with).expect("a plain header"),
    );

    headers
}

fn a_missing_idea() -> ServiceError<AlreadyCaptured> {
    ServiceError::NotFound {
        aggregate: AggregateId::from("idea_1"),
        kind: KIND,
    }
}

#[test]
fn no_if_match_demands_nothing() {
    // expect: no if-match demands no version
    assert_eq!(demanded(&HeaderMap::new()), Ok(None));
}

#[test]
fn a_quoted_version_is_demanded() {
    // expect: a quoted version is demanded
    assert_eq!(demanded(&an_if_match("\"7\"")), Ok(Some(Version::of(7))));
}

#[test]
fn an_unquoted_version_is_accepted_too() {
    // expect: an unquoted version is demanded as well
    assert_eq!(demanded(&an_if_match("7")), Ok(Some(Version::of(7))));
}

#[test]
fn surrounding_space_is_ignored() {
    // expect: a version amid spaces is demanded as without them
    assert_eq!(
        demanded(&an_if_match("  \"7\"  ")),
        Ok(Some(Version::of(7)))
    );
}

#[test]
fn a_star_demands_nothing_in_particular() {
    // expect: a star demands no version
    assert_eq!(
        demanded(&an_if_match("*")),
        Ok(None),
        "any version will do, which is the same as not asking"
    );
}

#[test]
fn an_if_match_that_is_not_a_version_is_refused() {
    // expect: words, nothing and a negative number are all unreadable
    assert_eq!(demanded(&an_if_match("\"not-a-version\"")), Err(Unreadable));
    assert_eq!(demanded(&an_if_match("\"\"")), Err(Unreadable));
    assert_eq!(demanded(&an_if_match("\"-1\"")), Err(Unreadable));
}

#[test]
fn a_tag_is_the_version_in_quotes() {
    // expect: a version's tag is it in quotes
    assert_eq!(tag(Version::of(7)), "\"7\"");
}

#[test]
fn a_tag_reads_back_as_the_version_it_came_from() {
    // given: a version
    let version = Version::of(42);

    // then: its tag asked for demands it
    assert_eq!(demanded(&an_if_match(&tag(version))), Ok(Some(version)));
}

async fn told(problem: Problem) -> (StatusCode, Value) {
    let response = problem.into_response();
    let status = response.status();
    assert_eq!(response.headers()[CONTENT_TYPE], PROBLEM);
    let body = body::to_bytes(response.into_body(), usize::MAX)
        .await
        .expect("a body");

    (
        status,
        serde_json::from_slice(&body).expect("a problem document"),
    )
}

#[tokio::test]
async fn a_problem_names_its_kind_and_tells_its_status_detail_and_values() {
    // when: a rejection with values is told
    let (status, told) = told(
        Problem::rejected("name-too-long", "a name must be at most 200 characters")
            .with("max", 200)
            .with("actual", 240),
    )
    .await;

    // then: it is a bad request naming its kind, detail and values
    assert_eq!(status, StatusCode::BAD_REQUEST);
    assert_eq!(
        told,
        json!({
            "type": "name-too-long",
            "status": 400,
            "detail": "a name must be at most 200 characters",
            "max": 200,
            "actual": 240,
        })
    );
}

#[tokio::test]
async fn nobody_known_is_unauthorized_and_somebody_not_allowed_forbidden() {
    // when: nobody known and somebody not allowed are told
    let (unknown, _) = told(Problem::unauthorized("not-signed-in", "sign in first")).await;
    let (unallowed, _) = told(Problem::forbidden("address-not-verified", "verify first")).await;

    // then: the one is unauthorized and the other forbidden
    assert_eq!(unknown, StatusCode::UNAUTHORIZED);
    assert_eq!(unallowed, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_throttled_problem_says_how_long_to_wait() {
    // when: a throttled problem is answered
    let response = Problem::throttled("too-many-tries", "wait a minute", 61).into_response();

    // then: it is too many requests, saying how long to wait
    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()[RETRY_AFTER], "61");
}

#[tokio::test]
async fn a_wait_is_told_in_whole_seconds_rounded_up_and_in_minutes() {
    // given: waits on and around a whole minute
    for (wait, seconds, minutes) in [
        (Duration::seconds(60), "60", 1),
        (Duration::milliseconds(59_500), "60", 1),
        (Duration::seconds(61), "61", 2),
    ] {
        // when: a wait is answered
        let response = Problem::waiting("too-many-tries", "wait", wait).into_response();

        // then: it is told in whole seconds rounded up
        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response.headers()[RETRY_AFTER],
            seconds,
            "told as fewer seconds than are left, a client knocks too early"
        );
        // and: in minutes rounded up
        let (_, told) = told(Problem::waiting("too-many-tries", "wait", wait)).await;
        assert_eq!(told["minutes"], minutes);
    }
}

#[tokio::test]
async fn a_problem_unthrottled_says_nothing_of_waiting() {
    // when: an unthrottled problem is answered
    let response = Problem::forbidden("address-not-verified", "verify first").into_response();

    // then: it says nothing of waiting
    assert!(response.headers().get(RETRY_AFTER).is_none());
}

#[tokio::test]
async fn a_value_cannot_pass_itself_off_as_the_kind() {
    // when: a problem with a value named as its kind is told
    let (_, told) =
        told(Problem::rejected("name-too-long", "too long").with("type", "other")).await;

    // then: its kind stays its own
    assert_eq!(told["type"], "name-too-long");
}

#[tokio::test]
async fn something_that_is_not_there_is_a_problem_not_found() {
    // when: a missing idea is told
    let (status, told) = told(Problem::of(&a_missing_idea(), |_| unreachable!())).await;

    // then: it is not found, saying what is missing
    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(told["type"], NOT_FOUND);
    assert_eq!(told["detail"], "there is no idea with id idea_1");
}

#[tokio::test]
async fn a_domain_refusal_is_named_by_the_feature() {
    // when: a refusal the feature names is told
    let (status, told) = told(Problem::of(
        &ServiceError::Refused(AlreadyCaptured),
        |refusal| Problem::refused("idea-already-captured", refusal),
    ))
    .await;

    // then: it is a conflict of that name, telling the refusal
    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(told["type"], "idea-already-captured");
    assert_eq!(told["detail"], "an idea cannot be captured twice");
}

#[tokio::test]
async fn a_stale_version_is_a_problem_outdated() {
    // given: a stale version
    let outdated: ServiceError<AlreadyCaptured> = ServiceError::Store(StoreError::Outdated {
        aggregate: AggregateId::from("idea_1"),
        kind: KIND,
        expected: Version::of(1),
    });

    // when: it is told
    let (status, told) = told(Problem::of(&outdated, |_| unreachable!())).await;

    // then: it is a failed precondition named outdated
    assert_eq!(status, StatusCode::PRECONDITION_FAILED);
    assert_eq!(told["type"], OUTDATED);
}

#[tokio::test]
async fn a_problem_we_cannot_explain_stays_unexplained() {
    // given: a problem we cannot explain
    let broken: ServiceError<AlreadyCaptured> = ServiceError::Unusable {
        aggregate: AggregateId::from("idea_1"),
        kind: KIND,
    };

    // when: it is told
    let (status, told) = told(Problem::of(&broken, |_| unreachable!())).await;

    // then: it is an internal error that says only something went wrong
    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(told["type"], UNEXPECTED);
    assert_eq!(told["detail"], "something went wrong");
}
