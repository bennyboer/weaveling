use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::header::ETAG;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use clock::text::serialize;
use eventsourcing::{Agent, Standing, Version};
use projects_contract::{CreateProjectRequest, ProjectDTO, RenameProjectRequest};
use projects_core::{Project, ProjectService, ProjectServiceError, ProjectSummary};
use serving::{Unreadable, demanded, refusal, tag};

pub fn router(projects: ProjectService) -> Router {
    Router::new()
        .route("/projects", get(list).post(create))
        .route("/projects/{id}", get(find).patch(rename).delete(remove))
        .with_state(projects)
}

async fn list(State(projects): State<ProjectService>) -> Result<Json<Vec<ProjectDTO>>, ApiError> {
    let found = projects.list().await?;

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
    Json(request): Json<CreateProjectRequest>,
) -> Result<Response, ApiError> {
    let id = projects
        .start(&request.name, &nobody_yet())
        .await?
        .to_string();
    let started = projects.get(&id).await?;

    Ok(to_response(StatusCode::CREATED, &id, &started))
}

async fn find(
    State(projects): State<ProjectService>,
    Path(id): Path<String>,
) -> Result<Response, ApiError> {
    let found = projects.get(&id).await?;

    Ok(to_response(StatusCode::OK, &id, &found))
}

async fn rename(
    State(projects): State<ProjectService>,
    Path(id): Path<String>,
    headers: HeaderMap,
    Json(request): Json<RenameProjectRequest>,
) -> Result<Response, ApiError> {
    projects
        .rename(&id, &request.name, expected(&headers)?, &nobody_yet())
        .await?;

    Ok(to_response(StatusCode::OK, &id, &projects.get(&id).await?))
}

async fn remove(
    State(projects): State<ProjectService>,
    Path(id): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, ApiError> {
    projects
        .delete(&id, expected(&headers)?, &nobody_yet())
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

fn nobody_yet() -> Agent {
    Agent::Anonymous
}

fn expected(headers: &HeaderMap) -> Result<Option<Version>, ApiError> {
    Ok(demanded(headers)?)
}

fn to_response(status: StatusCode, id: &str, standing: &Standing<Project>) -> Response {
    let project = &standing.state;

    (
        status,
        [(ETAG, tag(standing.version))],
        Json(ProjectDTO {
            id: id.to_owned(),
            version: standing.version.count(),
            name: project.name().to_string(),
            created_at: serialize(project.created_at()),
            updated_at: serialize(project.updated_at()),
        }),
    )
        .into_response()
}

enum ApiError {
    Unreadable(Unreadable),
    Refused(ProjectServiceError),
}

impl From<ProjectServiceError> for ApiError {
    fn from(error: ProjectServiceError) -> Self {
        Self::Refused(error)
    }
}

impl From<Unreadable> for ApiError {
    fn from(error: Unreadable) -> Self {
        Self::Unreadable(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let refused = match self {
            Self::Unreadable(reason) => {
                return (StatusCode::BAD_REQUEST, reason.to_string()).into_response();
            }
            Self::Refused(refused) => refused,
        };

        let (status, message) = match refused {
            ProjectServiceError::InvalidId(reason) => (StatusCode::BAD_REQUEST, reason.to_string()),
            ProjectServiceError::InvalidName(reason) => {
                (StatusCode::BAD_REQUEST, reason.to_string())
            }
            ProjectServiceError::Events(events) => refusal(&events),
            unserveable => {
                tracing::error!(error = %unserveable, "a project request could not be served");

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "something went wrong".to_owned(),
                )
            }
        };

        (status, message).into_response()
    }
}
