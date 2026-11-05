use gloo_net::http::Request;
use projects_contract::refused::TRASHED;
use projects_contract::{
    ChangeDetailsRequest, CreateProjectRequest, ProjectDTO, ProjectDetailsDTO,
    RenameProjectRequest, TrashedProjectDTO,
};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::http;
use crate::http::{ApiError, Refusal};
use crate::projects::model::{
    DescribedProject, Project, ProjectDetails, ProjectId, TrashedProject, Version,
};
use quotation::QuotationMarks;

const PROJECTS: &str = "/api/projects";
const SUBJECT: &str = "project";
const IF_MATCH: &str = "If-Match";

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

pub async fn one(id: &ProjectId) -> Result<Project, ApiError> {
    let response = http::sent(Request::get(&format!("{PROJECTS}/{id}"))).await?;

    as_listed(http::parsed(response, SUBJECT).await?)
}

/// One listed project read again, refused as moved to the trash rather than as unknown when that is where it went.
pub async fn still_listed(id: &ProjectId) -> Result<Project, ApiError> {
    match one(id).await {
        Err(ApiError::NotFound(subject)) => match in_the_trash().await {
            Ok(trashed) if trashed.iter().any(|held| &held.id == id) => Err(moved_to_the_trash()),
            _ => Err(ApiError::NotFound(subject)),
        },
        read => read,
    }
}

pub fn moved_to_the_trash() -> ApiError {
    ApiError::Rejected(Refusal {
        kind: Some(TRASHED.to_owned()),
        detail: String::new(),
        particulars: Vec::new(),
    })
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
        Request::put(&format!("{PROJECTS}/{id}/details")).header(IF_MATCH, &tag(saved_against)),
        &payload,
    )
    .await?;

    Ok(as_described(http::parsed(response, SUBJECT).await?))
}

pub async fn rename(id: &ProjectId, name: &str, on: Version) -> Result<Project, ApiError> {
    let payload = RenameProjectRequest {
        name: name.to_owned(),
    };
    let response = http::sent_with(
        Request::patch(&format!("{PROJECTS}/{id}")).header(IF_MATCH, &tag(on)),
        &payload,
    )
    .await?;

    as_project(http::parsed(response, SUBJECT).await?)
}

pub async fn trash(id: &ProjectId, on: Version) -> Result<(), ApiError> {
    let response =
        http::sent(Request::delete(&format!("{PROJECTS}/{id}")).header(IF_MATCH, &tag(on))).await?;

    http::checked(response, SUBJECT).await.map(|_| ())
}

pub async fn in_the_trash() -> Result<Vec<TrashedProject>, ApiError> {
    let response = http::sent(Request::get(&format!("{PROJECTS}/trash"))).await?;
    let trashed: Vec<TrashedProjectDTO> = http::parsed(response, SUBJECT).await?;

    trashed.into_iter().map(as_trashed).collect()
}

pub async fn restore(id: &ProjectId, on: Version) -> Result<(), ApiError> {
    let response =
        http::sent(Request::post(&format!("{PROJECTS}/{id}/restore")).header(IF_MATCH, &tag(on)))
            .await?;

    http::checked(response, SUBJECT).await.map(|_| ())
}

fn as_trashed(dto: TrashedProjectDTO) -> Result<TrashedProject, ApiError> {
    Ok(TrashedProject {
        id: ProjectId::from(dto.id),
        version: Version::from(dto.version),
        name: dto.name,
        until: OffsetDateTime::parse(&dto.until, &Rfc3339).map_err(|_| ApiError::Unexpected)?,
        restored: false,
    })
}

fn as_project(dto: ProjectDTO) -> Result<Project, ApiError> {
    let updated_at =
        OffsetDateTime::parse(&dto.updated_at, &Rfc3339).map_err(|_| ApiError::Unexpected)?;

    Ok(Project {
        id: ProjectId::from(dto.id),
        version: Version::from(dto.version),
        name: dto.name,
        updated_at,
        trashed: false,
    })
}

fn as_listed(found: ProjectDetailsDTO) -> Result<Project, ApiError> {
    as_project(ProjectDTO {
        id: found.id,
        version: found.version,
        name: found.name,
        created_at: found.created_at,
        updated_at: found.updated_at,
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

fn tag(on: Version) -> String {
    format!("\"{on}\"")
}
