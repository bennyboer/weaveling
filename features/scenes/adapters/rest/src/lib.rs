use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{delete, get, post};
use scenes_contract::{CreateSceneRequest, LinkIdeaRequest, RetitleSceneRequest, SceneDTO};
use scenes_core::{Scene, SceneService, SceneServiceError, StoreError};
use serde::Deserialize;

pub fn router(scenes: SceneService) -> Router {
    Router::new()
        .route("/scenes", get(list).post(create))
        .route("/scenes/{id}", get(find).patch(retitle).delete(remove))
        .route("/scenes/{id}/ideas", post(link))
        .route("/scenes/{id}/ideas/{idea}", delete(unlink))
        .with_state(scenes)
}

const AT_MOST: usize = 500;

#[derive(Deserialize)]
struct InProject {
    project: String,
}

async fn list(
    State(scenes): State<SceneService>,
    Query(asked): Query<InProject>,
) -> Result<Json<Vec<SceneDTO>>, ApiError> {
    let found = scenes.in_project(&asked.project, None, AT_MOST).await?;
    let mut listed = Vec::with_capacity(found.len());

    for id in found {
        listed.push(to_dto(&scenes.open(&id.to_string()).await?));
    }

    Ok(Json(listed))
}

async fn create(
    State(scenes): State<SceneService>,
    Json(request): Json<CreateSceneRequest>,
) -> Result<(StatusCode, Json<SceneDTO>), ApiError> {
    let created = scenes.create(&request.project).await?;

    Ok((StatusCode::CREATED, Json(to_dto(&created))))
}

async fn find(
    State(scenes): State<SceneService>,
    Path(id): Path<String>,
) -> Result<Json<SceneDTO>, ApiError> {
    let found = scenes.open(&id).await?;

    Ok(Json(to_dto(&found)))
}

async fn retitle(
    State(scenes): State<SceneService>,
    Path(id): Path<String>,
    Json(request): Json<RetitleSceneRequest>,
) -> Result<Json<SceneDTO>, ApiError> {
    let retitled = scenes.retitle(&id, &request.title).await?;

    Ok(Json(to_dto(&retitled)))
}

async fn link(
    State(scenes): State<SceneService>,
    Path(id): Path<String>,
    Json(request): Json<LinkIdeaRequest>,
) -> Result<Json<SceneDTO>, ApiError> {
    let linked = scenes.link(&id, &request.idea).await?;

    Ok(Json(to_dto(&linked)))
}

async fn unlink(
    State(scenes): State<SceneService>,
    Path((id, idea)): Path<(String, String)>,
) -> Result<Json<SceneDTO>, ApiError> {
    let unlinked = scenes.unlink(&id, &idea).await?;

    Ok(Json(to_dto(&unlinked)))
}

async fn remove(
    State(scenes): State<SceneService>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    scenes.delete(&id).await?;

    Ok(StatusCode::NO_CONTENT)
}

fn to_dto(scene: &Scene) -> SceneDTO {
    SceneDTO {
        id: scene.id().to_string(),
        project: scene.project().to_string(),
        title: scene.title().to_string(),
        ideas: scene.ideas().iter().map(ToString::to_string).collect(),
        text: scene.text(),
    }
}

struct ApiError(SceneServiceError);

impl From<SceneServiceError> for ApiError {
    fn from(error: SceneServiceError) -> Self {
        Self(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self.0 {
            SceneServiceError::InvalidId(error) => (StatusCode::BAD_REQUEST, error.to_string()),
            SceneServiceError::Untitled(error) => {
                (StatusCode::UNPROCESSABLE_ENTITY, error.to_string())
            }
            SceneServiceError::Store(StoreError::NotFound(id)) => {
                (StatusCode::NOT_FOUND, format!("scene {id} was not found"))
            }
            SceneServiceError::Store(StoreError::Conflict(id)) => {
                (StatusCode::CONFLICT, format!("scene {id} already exists"))
            }
            SceneServiceError::Store(error @ StoreError::Unusable(_)) => {
                (StatusCode::BAD_REQUEST, error.to_string())
            }
            SceneServiceError::Store(error @ StoreError::Backend(_)) => {
                tracing::error!(%error, "the scene store failed");

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "the scene store is unavailable".to_owned(),
                )
            }
        };

        (status, message).into_response()
    }
}
