use axum::Extension;
use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::header::ETAG;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, put};
use clock::text::serialize;
use eventsourcing::{Agent, Standing};
use live::{Broadcaster, followed_at};
use projects_contract::{
    ChangeDetailsRequest, CreateProjectRequest, ProjectDTO, ProjectDetailsDTO,
    RenameProjectRequest, refused,
};
use projects_core::{
    InvalidBookLanguage, InvalidProjectDescription, InvalidProjectName, InvalidQuotationMarks,
    Project, ProjectError, ProjectService, ProjectServiceError, ProjectSummary, RawDetails,
};
use serving::{Problem, demanded, tag};

pub const FOLLOWED: &str = "projects";

pub fn live(broadcaster: Broadcaster) -> Router {
    followed_at("/projects/live", broadcaster, |asked| {
        Some(
            asked
                .get("project")
                .cloned()
                .unwrap_or_else(|| FOLLOWED.to_owned()),
        )
    })
}

pub fn router(projects: ProjectService) -> Router {
    Router::new()
        .route("/projects", get(list).post(create))
        .route("/projects/{id}", get(find).patch(rename).delete(remove))
        .route("/projects/{id}/details", put(change_details))
        .with_state(projects)
}

async fn list(State(projects): State<ProjectService>) -> Result<Json<Vec<ProjectDTO>>, Problem> {
    let found = projects.list().await.map_err(problem_of)?;

    Ok(Json(found.iter().map(to_dto).collect()))
}

fn to_dto(summary: &ProjectSummary) -> ProjectDTO {
    ProjectDTO {
        id: summary.id.to_string(),
        version: summary.version.count(),
        name: summary.name.to_string(),
        created_at: serialize(summary.created_at),
        updated_at: serialize(summary.updated_at),
    }
}

async fn create(
    State(projects): State<ProjectService>,
    Extension(acting): Extension<Agent>,
    Json(request): Json<CreateProjectRequest>,
) -> Result<Response, Problem> {
    let id = projects
        .start(
            &request.name,
            &RawDetails {
                language: request.language.as_deref(),
                quotation: request.quotation.as_deref(),
                description: &request.description,
            },
            &acting,
        )
        .await
        .map_err(problem_of)?
        .to_string();
    let started = projects.get(&id).await.map_err(problem_of)?;

    Ok(to_response(StatusCode::CREATED, &id, &started))
}

async fn find(
    State(projects): State<ProjectService>,
    Path(id): Path<String>,
) -> Result<Response, Problem> {
    let found = projects.get(&id).await.map_err(problem_of)?;

    Ok(to_response(StatusCode::OK, &id, &found))
}

async fn rename(
    State(projects): State<ProjectService>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RenameProjectRequest>,
) -> Result<Response, Problem> {
    projects
        .rename(&id, &request.name, demanded(&headers)?, &acting)
        .await
        .map_err(problem_of)?;

    Ok(to_response(
        StatusCode::OK,
        &id,
        &projects.get(&id).await.map_err(problem_of)?,
    ))
}

async fn change_details(
    State(projects): State<ProjectService>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ChangeDetailsRequest>,
) -> Result<Response, Problem> {
    projects
        .change_details(
            &id,
            &RawDetails {
                language: request.language.as_deref(),
                quotation: request.quotation.as_deref(),
                description: &request.description,
            },
            demanded(&headers)?,
            &acting,
        )
        .await
        .map_err(problem_of)?;

    Ok(to_response(
        StatusCode::OK,
        &id,
        &projects.get(&id).await.map_err(problem_of)?,
    ))
}

async fn remove(
    State(projects): State<ProjectService>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    projects
        .delete(&id, demanded(&headers)?, &acting)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

fn to_response(status: StatusCode, id: &str, standing: &Standing<Project>) -> Response {
    let project = &standing.state;

    (
        status,
        [(ETAG, tag(standing.version))],
        Json(ProjectDetailsDTO {
            id: id.to_owned(),
            version: standing.version.count(),
            name: project.name().to_string(),
            language: project.details().language.as_ref().map(ToString::to_string),
            quotation: project.details().quotation.map(|style| style.to_string()),
            description: project.details().description.to_string(),
            created_at: serialize(project.created_at()),
            updated_at: serialize(project.updated_at()),
        }),
    )
        .into_response()
}

fn problem_of(refusal: ProjectServiceError) -> Problem {
    match refusal {
        ProjectServiceError::InvalidId(reason) => Problem::rejected(refused::ID_INVALID, reason),
        ProjectServiceError::InvalidName(reason) => match reason {
            InvalidProjectName::Blank => Problem::rejected(refused::NAME_BLANK, reason),
            InvalidProjectName::ControlCharacter => {
                Problem::rejected(refused::NAME_CONTROL_CHARACTER, reason)
            }
            InvalidProjectName::TooLong { max, actual } => {
                Problem::rejected(refused::NAME_TOO_LONG, reason)
                    .with("max", max)
                    .with("actual", actual)
            }
        },
        ProjectServiceError::InvalidLanguage(reason) => match &reason {
            InvalidBookLanguage::NotATag(tag) => {
                Problem::rejected(refused::LANGUAGE_NOT_A_TAG, &reason).with("tag", tag.as_str())
            }
        },
        ProjectServiceError::InvalidQuotation(reason) => match &reason {
            InvalidQuotationMarks::Unknown(name) => {
                Problem::rejected(refused::QUOTATION_UNKNOWN, &reason).with("name", name.as_str())
            }
        },
        ProjectServiceError::InvalidDescription(reason) => match reason {
            InvalidProjectDescription::Blank => Problem::unexpected(reason),
            InvalidProjectDescription::ControlCharacter => {
                Problem::rejected(refused::DESCRIPTION_CONTROL_CHARACTER, reason)
            }
            InvalidProjectDescription::TooLong { max, actual } => {
                Problem::rejected(refused::DESCRIPTION_TOO_LONG, reason)
                    .with("max", max)
                    .with("actual", actual)
            }
        },
        ProjectServiceError::Events(events) => Problem::of(&events, |refusal| match refusal {
            ProjectError::Deleted => Problem::refused(refused::DELETED, refusal),
            beyond_reach => Problem::unexpected(beyond_reach),
        }),
        unserveable => Problem::unexpected(unserveable),
    }
}
