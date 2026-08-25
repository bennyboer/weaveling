use gloo_net::http::Response;
use serde::de::DeserializeOwned;
use thiserror::Error;

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ApiError {
    #[error("{0}")]
    Rejected(String),
    #[error("That {0} no longer exists.")]
    NotFound(&'static str),
    #[error("Could not reach the server. Is it running?")]
    Offline,
    #[error("Something unexpected went wrong. Please try again.")]
    Unexpected,
}

pub(crate) async fn parsed<T: DeserializeOwned>(
    response: Response,
    subject: &'static str,
) -> Result<T, ApiError> {
    checked(response, subject)
        .await?
        .json()
        .await
        .map_err(|_| ApiError::Unexpected)
}

pub(crate) async fn checked(
    response: Response,
    subject: &'static str,
) -> Result<Response, ApiError> {
    if response.ok() {
        return Ok(response);
    }

    match response.status() {
        400 => Err(rejection(response).await),
        404 => Err(ApiError::NotFound(subject)),
        _ => Err(ApiError::Unexpected),
    }
}

async fn rejection(response: Response) -> ApiError {
    match response.text().await {
        Ok(detail) if !detail.trim().is_empty() => ApiError::Rejected(detail),
        _ => ApiError::Unexpected,
    }
}
