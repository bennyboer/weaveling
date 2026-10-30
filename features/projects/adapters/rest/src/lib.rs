use axum::Extension;
use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::header::ETAG;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing;
use clock::text;
use eventsourcing::{Agent, Standing};
use projects_contract::{
    ChangeDetailsRequest, CreateProjectRequest, ProjectDTO, ProjectDetailsDTO,
    RenameProjectRequest, refused,
};
use projects_core::{
    InvalidBookLanguage, InvalidProjectDescription, InvalidProjectName, InvalidQuotationMarks,
    Project, ProjectError, ProjectSummary, ProjectsCore, ProjectsError, RawDetails,
};
use serving::Problem;

mod following;

pub use following::{ProjectChanges, live};

pub fn router(projects: ProjectsCore) -> Router {
    Router::new()
        .route("/projects", routing::get(list).post(create))
        .route(
            "/projects/{id}",
            routing::get(find).patch(rename).delete(remove),
        )
        .route("/projects/{id}/details", routing::put(change_details))
        .with_state(projects)
}

async fn list(
    State(projects): State<ProjectsCore>,
    Extension(acting): Extension<Agent>,
) -> Result<Json<Vec<ProjectDTO>>, Problem> {
    let found = projects.list(&acting).await.map_err(problem_of)?;

    Ok(Json(found.iter().map(to_dto).collect()))
}

fn to_dto(summary: &ProjectSummary) -> ProjectDTO {
    ProjectDTO {
        id: summary.id.to_string(),
        version: summary.version.count(),
        name: summary.name.to_string(),
        created_at: text::serialize(summary.created_at),
        updated_at: text::serialize(summary.updated_at),
    }
}

async fn create(
    State(projects): State<ProjectsCore>,
    Extension(acting): Extension<Agent>,
    Json(request): Json<CreateProjectRequest>,
) -> Result<Response, Problem> {
    let (id, started) = projects
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
        .map_err(problem_of)?;

    Ok(to_response(StatusCode::CREATED, &id.to_string(), &started))
}

async fn find(
    State(projects): State<ProjectsCore>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
) -> Result<Response, Problem> {
    let found = projects.read(&id, &acting).await.map_err(problem_of)?;

    Ok(to_response(StatusCode::OK, &id, &found))
}

async fn rename(
    State(projects): State<ProjectsCore>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RenameProjectRequest>,
) -> Result<Response, Problem> {
    let renamed = projects
        .rename(&id, &request.name, serving::demanded(&headers)?, &acting)
        .await
        .map_err(problem_of)?;

    Ok(to_response(StatusCode::OK, &id, &renamed))
}

async fn change_details(
    State(projects): State<ProjectsCore>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<ChangeDetailsRequest>,
) -> Result<Response, Problem> {
    let changed = projects
        .change_details(
            &id,
            &RawDetails {
                language: request.language.as_deref(),
                quotation: request.quotation.as_deref(),
                description: &request.description,
            },
            serving::demanded(&headers)?,
            &acting,
        )
        .await
        .map_err(problem_of)?;

    Ok(to_response(StatusCode::OK, &id, &changed))
}

async fn remove(
    State(projects): State<ProjectsCore>,
    Extension(acting): Extension<Agent>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    projects
        .delete(&id, serving::demanded(&headers)?, &acting)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

fn to_response(status: StatusCode, id: &str, standing: &Standing<Project>) -> Response {
    let project = &standing.state;

    (
        status,
        [(ETAG, serving::tag(standing.version))],
        Json(ProjectDetailsDTO {
            id: id.to_owned(),
            version: standing.version.count(),
            name: project.name().to_string(),
            language: project.details().language.as_ref().map(ToString::to_string),
            quotation: project.details().quotation.map(|style| style.to_string()),
            description: project.details().description.to_string(),
            created_at: text::serialize(project.created_at()),
            updated_at: text::serialize(project.updated_at()),
        }),
    )
        .into_response()
}

fn problem_of(refusal: ProjectsError) -> Problem {
    match refusal {
        ProjectsError::InvalidId(reason) => Problem::rejected(refused::ID_INVALID, reason),
        ProjectsError::InvalidName(reason) => match reason {
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
        ProjectsError::InvalidLanguage(reason) => match &reason {
            InvalidBookLanguage::NotATag(tag) => {
                Problem::rejected(refused::LANGUAGE_NOT_A_TAG, &reason).with("tag", tag.as_str())
            }
        },
        ProjectsError::InvalidQuotation(reason) => match &reason {
            InvalidQuotationMarks::Unknown(name) => {
                Problem::rejected(refused::QUOTATION_UNKNOWN, &reason).with("name", name.as_str())
            }
        },
        ProjectsError::InvalidDescription(reason) => match reason {
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
        ProjectsError::Events(events) => Problem::of(&events, |refusal| match refusal {
            ProjectError::Deleted => Problem::refused(refused::DELETED, refusal),
            beyond_reach => Problem::unexpected(beyond_reach),
        }),
        ProjectsError::NotAnOwner => {
            Problem::forbidden(refused::NOT_AN_OWNER, ProjectsError::NotAnOwner)
        }
        unserveable => Problem::unexpected(unserveable),
    }
}
