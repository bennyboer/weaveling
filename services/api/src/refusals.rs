use std::sync::Arc;

use async_trait::async_trait;
use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use clock::Clock;
use clock::text::serialize;
use live::{Broadcaster, Teller, followed_at};
use messaging::{
    DeadLetter, Deliveries, DeliveryError, InProcessDispatcher, ListenerName, Message,
    Notifications, Queued,
};
use messaging_contract::RefusalDTO;
use serde_json::json;
use serving::Problem;
use time::OffsetDateTime;

const REFUSALS: &str = "refusals";

#[derive(Clone)]
struct Refusals {
    deliveries: Arc<dyn Deliveries>,
    dispatcher: Arc<InProcessDispatcher>,
    clock: Arc<dyn Clock>,
}

pub fn router(
    deliveries: Arc<dyn Deliveries>,
    dispatcher: Arc<InProcessDispatcher>,
    clock: Arc<dyn Clock>,
    broadcaster: Broadcaster,
) -> Router {
    Router::new()
        .route("/refusals", get(list))
        .route("/refusals/{id}/acknowledge", post(acknowledge))
        .route("/refusals/{id}/retry", post(retry))
        .with_state(Refusals {
            deliveries,
            dispatcher,
            clock,
        })
        .merge(followed_at("/refusals/live", broadcaster, |_| {
            Some(REFUSALS.to_owned())
        }))
}

pub struct TellingDeliveries {
    kept: Arc<dyn Deliveries>,
    teller: Teller,
}

impl TellingDeliveries {
    pub fn around(kept: Arc<dyn Deliveries>, broadcaster: Broadcaster) -> Self {
        Self {
            kept,
            teller: Teller::new(broadcaster),
        }
    }

    fn told(&self, outcome: Result<(), DeliveryError>) -> Result<(), DeliveryError> {
        if outcome.is_ok() {
            self.teller
                .tell(REFUSALS, REFUSALS, json!({ "name": "REFUSALS_CHANGED" }));
        }

        outcome
    }
}

#[async_trait]
impl Deliveries for TellingDeliveries {
    async fn enqueue(
        &self,
        listener: &ListenerName,
        message: &Message,
    ) -> Result<(), DeliveryError> {
        self.kept.enqueue(listener, message).await
    }

    async fn claim_due(
        &self,
        now: OffsetDateTime,
        at_most: i64,
    ) -> Result<Vec<Queued>, DeliveryError> {
        self.kept.claim_due(now, at_most).await
    }

    async fn mark_as_handled(&self, delivery: i64) -> Result<(), DeliveryError> {
        self.kept.mark_as_handled(delivery).await
    }

    async fn mark_as_refused(
        &self,
        delivery: i64,
        why: &str,
        again_at: OffsetDateTime,
    ) -> Result<(), DeliveryError> {
        self.kept.mark_as_refused(delivery, why, again_at).await
    }

    async fn give_up(
        &self,
        delivery: i64,
        why: &str,
        at: OffsetDateTime,
    ) -> Result<(), DeliveryError> {
        self.told(self.kept.give_up(delivery, why, at).await)
    }

    async fn dead_letters(&self) -> Result<Vec<DeadLetter>, DeliveryError> {
        self.kept.dead_letters().await
    }

    async fn retry(&self, dead_letter: i64, at: OffsetDateTime) -> Result<(), DeliveryError> {
        self.told(self.kept.retry(dead_letter, at).await)
    }

    async fn acknowledge(&self, dead_letter: i64, at: OffsetDateTime) -> Result<(), DeliveryError> {
        self.told(self.kept.acknowledge(dead_letter, at).await)
    }

    async fn waiting(&self) -> Result<usize, DeliveryError> {
        self.kept.waiting().await
    }

    async fn notifications(&self) -> Result<Box<dyn Notifications>, DeliveryError> {
        self.kept.notifications().await
    }
}

async fn list(State(refusals): State<Refusals>) -> Result<Json<Vec<RefusalDTO>>, Unserved> {
    let dead = refusals.deliveries.dead_letters().await?;

    Ok(Json(
        dead.iter()
            .map(|letter| to_dto(letter, &refusals.dispatcher))
            .collect(),
    ))
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

fn to_dto(dead: &DeadLetter, dispatcher: &InProcessDispatcher) -> RefusalDTO {
    RefusalDTO {
        id: dead.id,
        listener: dead.listener.to_string(),
        routing: dead.message.routing.to_string(),
        attempts: dead.attempts,
        why: dead.why.clone(),
        plainly: dispatcher.when_refused(&dead.listener).map(str::to_owned),
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
        Problem::unexpected(self.0).into_response()
    }
}
