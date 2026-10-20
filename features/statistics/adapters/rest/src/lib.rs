use std::sync::Arc;

use axum::extract::{Query, State};
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;
use serving::Problem;
use statistics_contract::WordCountDTO;
use statistics_core::{CatalogError, ProjectLink, WordCount, WordCountCatalog};

pub fn router(catalog: Arc<dyn WordCountCatalog>) -> Router {
    Router::new()
        .route("/statistics/word-counts", get(word_counts))
        .with_state(catalog)
}

#[derive(Deserialize)]
struct InProject {
    project: String,
}

async fn word_counts(
    State(catalog): State<Arc<dyn WordCountCatalog>>,
    Query(asked): Query<InProject>,
) -> Result<Json<Vec<WordCountDTO>>, ApiError> {
    let counts = catalog
        .in_project(&ProjectLink::from(asked.project))
        .await?;

    Ok(Json(counts.iter().map(to_dto).collect()))
}

fn to_dto(count: &WordCount) -> WordCountDTO {
    WordCountDTO {
        scene: count.scene.to_string(),
        words: count.words,
    }
}

struct ApiError(CatalogError);

impl From<CatalogError> for ApiError {
    fn from(error: CatalogError) -> Self {
        Self(error)
    }
}

impl IntoResponse for ApiError {
    fn into_response(self) -> Response {
        Problem::unexpected(self.0).into_response()
    }
}
