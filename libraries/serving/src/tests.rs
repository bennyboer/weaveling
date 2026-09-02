use axum::http::header::IF_MATCH;
use axum::http::{HeaderMap, HeaderValue, StatusCode};
use eventsourcing::{AggregateId, AggregateType, ServiceError, StoreError, Version};
use thiserror::Error;

use super::*;

const KIND: AggregateType = AggregateType::of("piece");

#[derive(Debug, Error)]
#[error("a piece cannot be captured twice")]
struct AlreadyCaptured;

fn asking(with: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        IF_MATCH,
        HeaderValue::from_str(with).expect("a plain header"),
    );

    headers
}

fn a_missing_piece() -> ServiceError<AlreadyCaptured> {
    ServiceError::NotFound {
        aggregate: AggregateId::from("piece_1"),
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

#[test]
fn something_that_is_not_there_is_not_found() {
    let (status, message) = refusal(&a_missing_piece());

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert_eq!(
        message, "there is no piece with id piece_1",
        "the aggregate says its own kind, so the wording cannot drift from the feature"
    );
}

#[test]
fn a_domain_refusal_is_a_conflict() {
    let (status, message) = refusal(&ServiceError::Refused(AlreadyCaptured));

    assert_eq!(status, StatusCode::CONFLICT);
    assert_eq!(message, "a piece cannot be captured twice");
}

#[test]
fn a_stale_version_fails_the_precondition() {
    let outdated: ServiceError<AlreadyCaptured> = ServiceError::Store(StoreError::Outdated {
        aggregate: AggregateId::from("piece_1"),
        kind: KIND,
        expected: Version::of(1),
    });

    let (status, message) = refusal(&outdated);

    assert_eq!(status, StatusCode::PRECONDITION_FAILED);
    assert_eq!(
        message,
        "this piece has moved on since the version you asked for"
    );
}

#[test]
fn anything_we_cannot_explain_stays_unexplained() {
    let broken: ServiceError<AlreadyCaptured> = ServiceError::Unusable {
        aggregate: AggregateId::from("piece_1"),
        kind: KIND,
    };

    let (status, message) = refusal(&broken);

    assert_eq!(status, StatusCode::INTERNAL_SERVER_ERROR);
    assert_eq!(
        message, "something went wrong",
        "a broken stream is our problem to fix, not the caller's to read about"
    );
}
