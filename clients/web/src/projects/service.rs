use gloo_net::http::Request;
use projects_contract::{
    ChangeDetailsRequest, CreateProjectRequest, ProjectDTO, ProjectDetailsDTO, RenameProjectRequest,
};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::http;
use crate::http::ApiError;
use crate::projects::model::{DescribedProject, Project, ProjectDetails, ProjectId, Version};
use quotation::QuotationMarks;

const PROJECTS: &str = "/api/projects";
const SUBJECT: &str = "project";

pub async fn list() -> Result<Vec<Project>, ApiError> {
    let response = http::sent(Request::get(PROJECTS)).await?;
    let listed: Vec<ProjectDTO> = http::parsed(response, SUBJECT).await?;

    listed.into_iter().map(as_project).collect()
}

pub async fn create(name: &str, details: &ProjectDetails) -> Result<Project, ApiError> {
    let payload = CreateProjectRequest {
        name: name.to_owned(),
        language: details.language.clone(),
        quotation: details.quotation.map(|style| style.name().to_owned()),
        description: details.description.clone(),
    };
    let response = http::sent_with(Request::post(PROJECTS), &payload).await?;

    as_project(http::parsed(response, SUBJECT).await?)
}

pub async fn details(id: &ProjectId) -> Result<DescribedProject, ApiError> {
    let response = http::sent(Request::get(&format!("{PROJECTS}/{id}"))).await?;

    Ok(as_described(http::parsed(response, SUBJECT).await?))
}

pub async fn change_details(
    id: &ProjectId,
    details: &ProjectDetails,
    saved_against: Version,
) -> Result<DescribedProject, ApiError> {
    let payload = ChangeDetailsRequest {
        language: details.language.clone(),
        quotation: details.quotation.map(|style| style.name().to_owned()),
        description: details.description.clone(),
    };
    let response = http::sent_with(
        Request::put(&format!("{PROJECTS}/{id}/details"))
            .header("If-Match", &format!("\"{saved_against}\"")),
        &payload,
    )
    .await?;

    Ok(as_described(http::parsed(response, SUBJECT).await?))
}

pub async fn rename(id: &ProjectId, name: &str) -> Result<Project, ApiError> {
    let payload = RenameProjectRequest {
        name: name.to_owned(),
    };
    let response = http::sent_with(Request::patch(&format!("{PROJECTS}/{id}")), &payload).await?;

    as_project(http::parsed(response, SUBJECT).await?)
}

pub async fn delete(id: &ProjectId) -> Result<(), ApiError> {
    let response = http::sent(Request::delete(&format!("{PROJECTS}/{id}"))).await?;

    http::checked(response, SUBJECT).await.map(|_| ())
}

fn as_project(dto: ProjectDTO) -> Result<Project, ApiError> {
    let updated_at =
        OffsetDateTime::parse(&dto.updated_at, &Rfc3339).map_err(|_| ApiError::Unexpected)?;

    Ok(Project {
        id: ProjectId::from(dto.id),
        version: Version::from(dto.version),
        name: dto.name,
        updated_at,
    })
}

fn as_described(found: ProjectDetailsDTO) -> DescribedProject {
    DescribedProject {
        version: Version::from(found.version),
        name: found.name,
        details: ProjectDetails {
            language: found.language,
            quotation: found.quotation.as_deref().and_then(QuotationMarks::named),
            description: found.description,
        },
    }
}
