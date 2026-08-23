use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use passages_contract::PassageDTO;
use passages_core::{Passage, PassageService, PassageServiceError, StoreError};

pub fn router(passages: PassageService) -> Router {
    Router::new()
        .route("/passages", post(create))
        .route("/passages/{id}", get(find).delete(remove))
        .with_state(passages)
}

async fn create(
    State(passages): State<PassageService>,
) -> Result<(StatusCode, Json<PassageDTO>), ApiError> {
    let created = passages.create().await?;

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
