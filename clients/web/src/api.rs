use gloo_net::http::{Request, Response};
use projects_contract::{CreateProjectRequest, ProjectDTO, RenameProjectRequest};
use serde::de::DeserializeOwned;
use thiserror::Error;

const PROJECTS: &str = "/api/projects";

#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum ApiError {
    #[error("{0}")]
    Rejected(String),
    #[error("That project no longer exists.")]
    NotFound,
    #[error("Could not reach the server. Is it running?")]
    Offline,
    #[error("Something unexpected went wrong. Please try again.")]
    Unexpected,
}

pub async fn list() -> Result<Vec<ProjectDTO>, ApiError> {
    let response = Request::get(PROJECTS)
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    parsed(response).await
}

pub async fn create(name: &str) -> Result<ProjectDTO, ApiError> {
    let payload = CreateProjectRequest {
        name: name.to_owned(),
    };
    let response = Request::post(PROJECTS)
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    parsed(response).await
}

pub async fn rename(id: &str, name: &str) -> Result<ProjectDTO, ApiError> {
    let payload = RenameProjectRequest {
        name: name.to_owned(),
    };
    let response = Request::patch(&format!("{PROJECTS}/{id}"))
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    parsed(response).await
}

pub async fn delete(id: &str) -> Result<(), ApiError> {
    let response = Request::delete(&format!("{PROJECTS}/{id}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    checked(response).await.map(|_| ())
}

async fn parsed<T: DeserializeOwned>(response: Response) -> Result<T, ApiError> {
    checked(response)
        .await?
        .json()
        .await
        .map_err(|_| ApiError::Unexpected)
}

async fn checked(response: Response) -> Result<Response, ApiError> {
    if response.ok() {
        return Ok(response);
    }

    match response.status() {
        400 => Err(rejection(response).await),
        404 => Err(ApiError::NotFound),
        _ => Err(ApiError::Unexpected),
    }
}

async fn rejection(response: Response) -> ApiError {
    match response.text().await {
        Ok(detail) if !detail.trim().is_empty() => ApiError::Rejected(detail),
        _ => ApiError::Unexpected,
    }
}
