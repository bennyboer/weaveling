use std::fmt::Display;

use axum::http::StatusCode;
use eventsourcing::{ServiceError, StoreError};

pub fn refusal<E: Display>(error: &ServiceError<E>) -> (StatusCode, String) {
    match error {
        ServiceError::NotFound { .. } => (StatusCode::NOT_FOUND, error.to_string()),
        ServiceError::Refused(refused) => (StatusCode::CONFLICT, refused.to_string()),
        ServiceError::Store(StoreError::Outdated { kind, .. }) => (
            StatusCode::PRECONDITION_FAILED,
            format!("this {kind} has moved on since the version you asked for"),
        ),
        unserveable => {
            tracing::error!(error = %unserveable, "a request could not be served");

            (
                StatusCode::INTERNAL_SERVER_ERROR,
                "something went wrong".to_owned(),
            )
        }
    }
}
