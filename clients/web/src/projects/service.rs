use gloo_net::http::Request;
use projects_contract::{CreateProjectRequest, ProjectDTO, RenameProjectRequest};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::http::{ApiError, checked, parsed};
use crate::projects::model::{Project, ProjectId};

const PROJECTS: &str = "/api/projects";
const SUBJECT: &str = "project";

pub async fn list() -> Result<Vec<Project>, ApiError> {
    let response = Request::get(PROJECTS)
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;
    let listed: Vec<ProjectDTO> = parsed(response, SUBJECT).await?;

    listed.into_iter().map(as_project).collect()
}

pub async fn create(name: &str) -> Result<Project, ApiError> {
    let payload = CreateProjectRequest {
        name: name.to_owned(),
    };
    let response = Request::post(PROJECTS)
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    as_project(parsed(response, SUBJECT).await?)
}

pub async fn rename(id: &ProjectId, name: &str) -> Result<Project, ApiError> {
    let payload = RenameProjectRequest {
        name: name.to_owned(),
    };
    let response = Request::patch(&format!("{PROJECTS}/{id}"))
        .json(&payload)
        .map_err(|_| ApiError::Unexpected)?
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    as_project(parsed(response, SUBJECT).await?)
}

pub async fn delete(id: &ProjectId) -> Result<(), ApiError> {
    let response = Request::delete(&format!("{PROJECTS}/{id}"))
        .send()
        .await
        .map_err(|_| ApiError::Offline)?;

    checked(response, SUBJECT).await.map(|_| ())
}

fn as_project(dto: ProjectDTO) -> Result<Project, ApiError> {
    let updated_at =
        OffsetDateTime::parse(&dto.updated_at, &Rfc3339).map_err(|_| ApiError::Unexpected)?;

    Ok(Project {
        id: ProjectId::from(dto.id),
        name: dto.name,
        updated_at,
    })
}
