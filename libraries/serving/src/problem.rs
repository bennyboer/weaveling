use std::fmt::Display;

use axum::Json;
use axum::http::HeaderValue;
use axum::http::StatusCode;
use axum::http::header::{CONTENT_TYPE, RETRY_AFTER};
use axum::response::{IntoResponse, Response};
use eventsourcing::{ServiceError, StoreError};
use serde_json::{Map, Value};

use crate::conditional::Unreadable;

pub const PROBLEM: &str = "application/problem+json";
pub const NOT_FOUND: &str = "not-found";
pub const OUTDATED: &str = "outdated";
pub const UNREADABLE: &str = "unreadable";
pub const UNEXPECTED: &str = "unexpected";

#[derive(Debug)]
pub struct Problem {
    status: StatusCode,
    kind: &'static str,
    detail: String,
    values: Map<String, Value>,
    retry_after: Option<u64>,
}

impl Problem {
    pub fn rejected(kind: &'static str, detail: impl Display) -> Self {
        Self::new(StatusCode::BAD_REQUEST, kind, detail)
    }

    pub fn refused(kind: &'static str, detail: impl Display) -> Self {
        Self::new(StatusCode::CONFLICT, kind, detail)
    }

    pub fn not_found(kind: &'static str, detail: impl Display) -> Self {
        Self::new(StatusCode::NOT_FOUND, kind, detail)
    }

    pub fn unprocessable(kind: &'static str, detail: impl Display) -> Self {
        Self::new(StatusCode::UNPROCESSABLE_ENTITY, kind, detail)
    }

    pub fn unauthorized(kind: &'static str, detail: impl Display) -> Self {
        Self::new(StatusCode::UNAUTHORIZED, kind, detail)
    }

    pub fn forbidden(kind: &'static str, detail: impl Display) -> Self {
        Self::new(StatusCode::FORBIDDEN, kind, detail)
    }

    pub fn throttled(kind: &'static str, detail: impl Display, retry_after_seconds: u64) -> Self {
        Self {
            retry_after: Some(retry_after_seconds),
            ..Self::new(StatusCode::TOO_MANY_REQUESTS, kind, detail)
        }
    }

    pub fn unexpected(error: impl Display) -> Self {
        tracing::error!(error = %error, "a request could not be served");

        Self::new(
            StatusCode::INTERNAL_SERVER_ERROR,
            UNEXPECTED,
            "something went wrong",
        )
    }

    pub fn of<E: Display>(error: &ServiceError<E>, refused: impl FnOnce(&E) -> Self) -> Self {
        match error {
            ServiceError::NotFound { .. } => Self::new(StatusCode::NOT_FOUND, NOT_FOUND, error),
            ServiceError::Refused(refusal) => refused(refusal),
            ServiceError::Store(StoreError::Outdated { kind, .. }) => Self::new(
                StatusCode::PRECONDITION_FAILED,
                OUTDATED,
                format!("this {kind} has moved on since the version you asked for"),
            ),
            unserveable => Self::unexpected(unserveable),
        }
    }

    pub fn with(mut self, name: &str, value: impl Into<Value>) -> Self {
        self.values.insert(name.to_owned(), value.into());

        self
    }

    fn new(status: StatusCode, kind: &'static str, detail: impl Display) -> Self {
        Self {
            status,
            kind,
            detail: detail.to_string(),
            values: Map::new(),
            retry_after: None,
        }
    }
}

impl From<Unreadable> for Problem {
    fn from(unreadable: Unreadable) -> Self {
        Self::rejected(UNREADABLE, unreadable)
    }
}

impl IntoResponse for Problem {
    fn into_response(self) -> Response {
        let mut told = self.values;
        told.insert("type".to_owned(), self.kind.into());
        told.insert("status".to_owned(), self.status.as_u16().into());
        told.insert("detail".to_owned(), self.detail.into());

        let mut response = (self.status, [(CONTENT_TYPE, PROBLEM)], Json(told)).into_response();
        if let Some(seconds) = self.retry_after {
            response
                .headers_mut()
                .insert(RETRY_AFTER, HeaderValue::from(seconds));
        }

        response
    }
}
