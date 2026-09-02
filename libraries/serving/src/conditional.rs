use axum::http::HeaderMap;
use axum::http::header::IF_MATCH;
use eventsourcing::Version;
use thiserror::Error;

const ANY: &str = "*";

#[derive(Debug, Clone, Copy, PartialEq, Eq, Error)]
#[error("If-Match must be a version, in quotes")]
pub struct Unreadable;

pub fn demanded(headers: &HeaderMap) -> Result<Option<Version>, Unreadable> {
    let Some(asked) = headers.get(IF_MATCH) else {
        return Ok(None);
    };
    let asked = asked.to_str().map_err(|_| Unreadable)?.trim();

    if asked == ANY {
        return Ok(None);
    }

    asked
        .trim_matches('"')
        .parse()
        .map(|counted: u64| Some(Version::of(counted)))
        .map_err(|_| Unreadable)
}

pub fn tag(version: Version) -> String {
    format!("\"{version}\"")
}
