use std::sync::Arc;

use appearances_contract::PlaceDTO;
use appearances_core::{AppearanceCatalog, CatalogError, IdeaLink, Place, Subject};
use axum::extract::{Query, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::get;
use axum::{Json, Router};
use serde::Deserialize;

pub fn router(catalog: Arc<dyn AppearanceCatalog>) -> Router {
    Router::new()
        .route("/appearances", get(places_of))
        .with_state(catalog)
}

#[derive(Deserialize)]
struct OfIdea {
    idea: String,
}

async fn places_of(
    State(catalog): State<Arc<dyn AppearanceCatalog>>,
    Query(asked): Query<OfIdea>,
) -> Result<Json<Vec<PlaceDTO>>, ApiError> {
    let places = catalog
        .places_of(&Subject::Idea(IdeaLink::from(asked.idea)))
        .await?;

    Ok(Json(places.iter().map(to_dto).collect()))
}

fn to_dto(place: &Place) -> PlaceDTO {
    match place {
        Place::Scene(scene) => PlaceDTO::Scene {
            id: scene.to_string(),
        },
        Place::Section(section) => PlaceDTO::Section {
            id: section.to_string(),
        },
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
        tracing::error!(error = %self.0, "the appearance catalog failed");

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "the appearance catalog is unavailable",
        )
            .into_response()
    }
}
