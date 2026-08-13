use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use projects_contract::{CreateProjectRequest, ProjectDTO, RenameProjectRequest};
use projects_core::{Project, ProjectError, ProjectService, StoreError};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

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

async fn create(
    State(projects): State<ProjectService>,
    Json(request): Json<CreateProjectRequest>,
) -> Result<(StatusCode, Json<ProjectDTO>), ApiError> {
    let created = projects.create(&request.name).await?;

    Ok((StatusCode::CREATED, Json(to_dto(&created))))
}

async fn find(
    State(projects): State<ProjectService>,
    Path(id): Path<String>,
) -> Result<Json<ProjectDTO>, ApiError> {
    let found = projects.get(&id).await?;

    Ok(Json(to_dto(&found)))
}

async fn rename(
    State(projects): State<ProjectService>,
    Path(id): Path<String>,
    Json(request): Json<RenameProjectRequest>,
) -> Result<Json<ProjectDTO>, ApiError> {
    let renamed = projects.rename(&id, &request.name).await?;

    Ok(Json(to_dto(&renamed)))
}

async fn remove(
    State(projects): State<ProjectService>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    projects.delete(&id).await?;

    Ok(StatusCode::NO_CONTENT)
}

fn to_dto(project: &Project) -> ProjectDTO {
    ProjectDTO {
        id: project.id().to_string(),
        name: project.name().to_string(),
        created_at: to_rfc3339(project.created_at()),
        updated_at: to_rfc3339(project.updated_at()),
    }
}

fn to_rfc3339(at: OffsetDateTime) -> String {
    at.format(&Rfc3339)
        .expect("a project timestamp should be representable as RFC 3339")
}

struct ApiError(ProjectError);

impl From<ProjectError> for ApiError {
    fn from(error: ProjectError) -> Self {
        Self(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self.0 {
            ProjectError::InvalidId(error) => (StatusCode::BAD_REQUEST, error.to_string()),
            ProjectError::InvalidName(error) => (StatusCode::BAD_REQUEST, error.to_string()),
            ProjectError::Store(StoreError::NotFound(id)) => {
                (StatusCode::NOT_FOUND, format!("project {id} was not found"))
            }
            ProjectError::Store(StoreError::Conflict(id)) => {
                (StatusCode::CONFLICT, format!("project {id} already exists"))
            }
            ProjectError::Store(error @ StoreError::Backend(_)) => {
                tracing::error!(%error, "the project store failed");

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "the project store is unavailable".to_owned(),
                )
            }
        };

        (status, message).into_response()
    }
}
