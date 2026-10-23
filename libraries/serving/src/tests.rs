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

fn asking(with: &str) -> HeaderMap {
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
    assert_eq!(demanded(&HeaderMap::new()), Ok(None));
}

#[test]
fn a_quoted_version_is_demanded() {
    assert_eq!(demanded(&asking("\"7\"")), Ok(Some(Version::of(7))));
}

#[test]
fn an_unquoted_version_is_accepted_too() {
    assert_eq!(demanded(&asking("7")), Ok(Some(Version::of(7))));
}

#[test]
fn surrounding_space_is_ignored() {
    assert_eq!(demanded(&asking("  \"7\"  ")), Ok(Some(Version::of(7))));
}

#[test]
fn a_star_demands_nothing_in_particular() {
    assert_eq!(
        demanded(&asking("*")),
        Ok(None),
        "any version will do, which is the same as not asking"
    );
}

#[test]
fn an_if_match_that_is_not_a_version_is_refused() {
    assert_eq!(demanded(&asking("\"not-a-version\"")), Err(Unreadable));
    assert_eq!(demanded(&asking("\"\"")), Err(Unreadable));
    assert_eq!(demanded(&asking("\"-1\"")), Err(Unreadable));
}

#[test]
fn a_tag_is_the_version_in_quotes() {
    assert_eq!(tag(Version::of(7)), "\"7\"");
}

#[test]
fn a_tag_reads_back_as_the_version_it_came_from() {
    let version = Version::of(42);

    assert_eq!(demanded(&asking(&tag(version))), Ok(Some(version)));
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
    let (status, told) = told(
        Problem::rejected("name-too-long", "a name must be at most 200 characters")
            .with("max", 200)
            .with("actual", 240),
    )
    .await;

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
    let (unknown, _) = told(Problem::unauthorized("not-signed-in", "sign in first")).await;
    let (unallowed, _) = told(Problem::forbidden("address-not-verified", "verify first")).await;

    assert_eq!(unknown, StatusCode::UNAUTHORIZED);
    assert_eq!(unallowed, StatusCode::FORBIDDEN);
}

#[tokio::test]
async fn a_throttled_problem_says_how_long_to_wait() {
    let response = Problem::throttled("too-many-tries", "wait a minute", 61).into_response();

    assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
    assert_eq!(response.headers()[RETRY_AFTER], "61");
}

#[tokio::test]
async fn a_wait_is_told_in_whole_seconds_rounded_up_and_in_minutes() {
    for (wait, seconds, minutes) in [
        (Duration::seconds(60), "60", 1),
        (Duration::milliseconds(59_500), "60", 1),
        (Duration::seconds(61), "61", 2),
    ] {
        let response = Problem::waiting("too-many-tries", "wait", wait).into_response();

        assert_eq!(response.status(), StatusCode::TOO_MANY_REQUESTS);
        assert_eq!(
            response.headers()[RETRY_AFTER],
            seconds,
            "told as fewer seconds than are left, a client knocks too early"
        );
        let (_, told) = told(Problem::waiting("too-many-tries", "wait", wait)).await;
        assert_eq!(told["minutes"], minutes);
    }
}

#[tokio::test]
async fn a_problem_unthrottled_says_nothing_of_waiting() {
    let response = Problem::forbidden("address-not-verified", "verify first").into_response();

    assert!(response.headers().get(RETRY_AFTER).is_none());
}

#[tokio::test]
async fn a_value_cannot_pass_itself_off_as_the_kind() {
    let (_, told) =
        told(Problem::rejected("name-too-long", "too long").with("type", "other")).await;

    assert_eq!(told["type"], "name-too-long");
}

#[tokio::test]
async fn something_that_is_not_there_is_a_problem_not_found() {
    let (status, told) = told(Problem::of(&a_missing_idea(), |_| unreachable!())).await;

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(told["type"], NOT_FOUND);
    assert_eq!(told["detail"], "there is no idea with id idea_1");
}

#[tokio::test]
async fn a_domain_refusal_is_named_by_the_feature() {
    let (status, told) = told(Problem::of(
        &ServiceError::Refused(AlreadyCaptured),
        |refusal| Problem::refused("idea-already-captured", refusal),
    ))
    .await;

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(told["type"], "idea-already-captured");
    assert_eq!(told["detail"], "an idea cannot be captured twice");
}

#[tokio::test]
async fn a_stale_version_is_a_problem_outdated() {
    let outdated: ServiceError<AlreadyCaptured> = ServiceError::Store(StoreError::Outdated {
        aggregate: AggregateId::from("idea_1"),
        kind: KIND,
        expected: Version::of(1),
    });

    let (status, told) = told(Problem::of(&outdated, |_| unreachable!())).await;

    assert_eq!(status, StatusCode::PRECONDITION_FAILED);
    assert_eq!(told["type"], OUTDATED);
}

#[tokio::test]
async fn a_problem_we_cannot_explain_stays_unexplained() {
    let broken: ServiceError<AlreadyCaptured> = ServiceError::Unusable {
        aggregate: AggregateId::from("idea_1"),
        kind: KIND,
    };

    let (status, told) = told(Problem::of(&broken, |_| unreachable!())).await;

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(told["type"], UNEXPECTED);
    assert_eq!(told["detail"], "something went wrong");
}
