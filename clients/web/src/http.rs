use gloo_net::http::{RequestBuilder, Response};
use serde::Serialize;
use serde::de::DeserializeOwned;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum ApiError {
    Rejected(String),
    NotFound(&'static str),
    Outdated(&'static str),
    Overtaken,
    Offline,
    Unexpected,
}

pub(crate) async fn sent(request: RequestBuilder) -> Result<Response, ApiError> {
    request.send().await.map_err(|_| ApiError::Offline)
}

pub(crate) async fn sent_with(
    request: RequestBuilder,
    body: &impl Serialize,
) -> Result<Response, ApiError> {
    request
        .json(body)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)
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
        412 => Err(ApiError::Outdated(subject)),
        _ => Err(ApiError::Unexpected),
    }
}

async fn rejection(response: Response) -> ApiError {
    match response.text().await {
        Ok(detail) if !detail.trim().is_empty() => ApiError::Rejected(detail),
        _ => ApiError::Unexpected,
    }
}
