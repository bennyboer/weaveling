use std::sync::Arc;

use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use clock::Clock;
use clock::text::serialize;
use messaging::{DeadLetter, Deliveries, DeliveryError};
use messaging_contract::RefusalDTO;

#[derive(Clone)]
struct Refusals {
    deliveries: Arc<dyn Deliveries>,
    clock: Arc<dyn Clock>,
}

pub fn router(deliveries: Arc<dyn Deliveries>, clock: Arc<dyn Clock>) -> Router {
    Router::new()
        .route("/refusals", get(list))
        .route("/refusals/{id}/acknowledge", post(acknowledge))
        .route("/refusals/{id}/retry", post(retry))
        .with_state(Refusals { deliveries, clock })
}

async fn list(State(refusals): State<Refusals>) -> Result<Json<Vec<RefusalDTO>>, Unserved> {
    let dead = refusals.deliveries.dead_letters().await?;

    Ok(Json(dead.iter().map(to_dto).collect()))
}

async fn retry(
    State(refusals): State<Refusals>,
    Path(id): Path<i64>,
) -> Result<StatusCode, Unserved> {
    refusals.deliveries.retry(id, refusals.clock.now()).await?;

    Ok(StatusCode::NO_CONTENT)
}

async fn acknowledge(
    State(refusals): State<Refusals>,
    Path(id): Path<i64>,
) -> Result<StatusCode, Unserved> {
    refusals
        .deliveries
        .acknowledge(id, refusals.clock.now())
        .await?;

    Ok(StatusCode::NO_CONTENT)
}

fn to_dto(dead: &DeadLetter) -> RefusalDTO {
    RefusalDTO {
        id: dead.id,
        listener: dead.listener.to_string(),
        routing: dead.message.routing.to_string(),
        attempts: dead.attempts,
        why: dead.why.clone(),
        occurred_at: serialize(dead.message.occurred_at),
        given_up_at: serialize(dead.given_up_at),
        acknowledged_at: dead.acknowledged_at.map(serialize),
    }
}

struct Unserved(DeliveryError);

impl From<DeliveryError> for Unserved {
    fn from(error: DeliveryError) -> Self {
        Self(error)
    }
}

impl IntoResponse for Unserved {
    fn into_response(self) -> Response {
        tracing::error!(error = %self.0, "a refusals request could not be served");

        (
            StatusCode::INTERNAL_SERVER_ERROR,
            "something went wrong".to_owned(),
        )
            .into_response()
    }
}
