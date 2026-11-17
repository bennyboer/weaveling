mod transformer;

use axum::extract::{Query, State};
use axum::routing;
use axum::{Extension, Json, Router};
use eventsourcing::Agent;
use readings_contract::ContentsDTO;
use readings_core::{ReadingsCore, ReadingsError};
use serde::Deserialize;
use serving::Problem;

pub fn router(readings: ReadingsCore) -> Router {
    Router::new()
        .route("/readings/live", routing::get(live_book))
        .with_state(readings)
}

#[derive(Deserialize)]
struct InProject {
    project: String,
}

async fn live_book(
    State(readings): State<ReadingsCore>,
    Extension(agent): Extension<Agent>,
    Query(asked): Query<InProject>,
) -> Result<Json<ContentsDTO>, Problem> {
    let book =
        readings
            .live_book(&asked.project, &agent)
            .await
            .map_err(|refusal| match refusal {
                ReadingsError::ProjectNotThere => Problem::not_found(serving::NOT_FOUND, refusal),
                beyond_reach => Problem::unexpected(beyond_reach),
            })?;

    Ok(Json(transformer::to_dto(&book)))
}
