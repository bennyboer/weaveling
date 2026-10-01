use axum::Json;
use axum::Router;
use axum::extract::{Path, Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use passages_contract::{CreatePassageRequest, PassageDTO};
use passages_core::{Passage, PassageService, PassageServiceError, StoreError};
use serde::Deserialize;

pub fn router(passages: PassageService) -> Router {
    Router::new()
        .route("/passages", get(list).post(create))
        .route("/passages/{id}", get(find).delete(remove))
        .with_state(passages)
}

const AT_MOST: usize = 500;

#[derive(Deserialize)]
struct InProject {
    project: String,
}

async fn list(
    State(passages): State<PassageService>,
    Query(asked): Query<InProject>,
) -> Result<Json<Vec<PassageDTO>>, ApiError> {
    let found = passages.in_project(&asked.project, None, AT_MOST).await?;
    let mut listed = Vec::with_capacity(found.len());

    for id in found {
        listed.push(to_dto(&passages.open(&id.to_string()).await?));
    }

    Ok(Json(listed))
}

async fn create(
    State(passages): State<PassageService>,
    Json(request): Json<CreatePassageRequest>,
) -> Result<(StatusCode, Json<PassageDTO>), ApiError> {
    let created = passages.create(&request.project).await?;

    Ok((StatusCode::CREATED, Json(to_dto(&created))))
}

async fn find(
    State(passages): State<PassageService>,
    Path(id): Path<String>,
) -> Result<Json<PassageDTO>, ApiError> {
    let found = passages.open(&id).await?;

    Ok(Json(to_dto(&found)))
}

async fn remove(
    State(passages): State<PassageService>,
    Path(id): Path<String>,
) -> Result<StatusCode, ApiError> {
    passages.delete(&id).await?;

    Ok(StatusCode::NO_CONTENT)
}

fn to_dto(passage: &Passage) -> PassageDTO {
    PassageDTO {
        id: passage.id().to_string(),
        project: passage.project().to_string(),
        text: passage.text(),
    }
}

struct ApiError(PassageServiceError);

impl From<PassageServiceError> for ApiError {
    fn from(error: PassageServiceError) -> Self {
        Self(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        let (status, message) = match self.0 {
            PassageServiceError::InvalidId(error) => (StatusCode::BAD_REQUEST, error.to_string()),
            PassageServiceError::Store(StoreError::NotFound(id)) => {
                (StatusCode::NOT_FOUND, format!("passage {id} was not found"))
            }
            PassageServiceError::Store(StoreError::Conflict(id)) => {
                (StatusCode::CONFLICT, format!("passage {id} already exists"))
            }
            PassageServiceError::Store(error @ StoreError::Unusable(_)) => {
                (StatusCode::BAD_REQUEST, error.to_string())
            }
            PassageServiceError::Store(error @ StoreError::Backend(_)) => {
                tracing::error!(%error, "the passage store failed");

                (
                    StatusCode::INTERNAL_SERVER_ERROR,
                    "the passage store is unavailable".to_owned(),
                )
            }
        };

        (status, message).into_response()
    }
}
