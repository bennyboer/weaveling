use std::sync::Arc;

use async_trait::async_trait;
use axum::Json;
use axum::Router;
use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing;
use clock::Clock;
use clock::text;
use live::{Broadcaster, Teller};
use messaging::{
    DeadLetter, Deliveries, DeliveryError, InProcessDispatcher, ListenerName, Message,
    Notifications, Queued,
};
use messaging_contract::{RefusalDTO, refused};
use serde_json::json;
use serving::Problem;
use time::OffsetDateTime;

const REFUSALS: &str = "refusals";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RefusalChanges;

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
    broadcaster: Broadcaster<RefusalChanges>,
) -> Router {
    Router::new()
        .route("/refusals", routing::get(list))
        .route("/refusals/{id}/acknowledge", routing::post(acknowledge))
        .route("/refusals/{id}/retry", routing::post(retry))
        .with_state(Refusals {
            deliveries,
            dispatcher,
            clock,
        })
        .merge(live::followed_at("/refusals/live", broadcaster, |_| {
            Some(RefusalChanges)
        }))
}

pub struct TellingDeliveries {
    kept: Arc<dyn Deliveries>,
    teller: Teller<RefusalChanges>,
}

impl TellingDeliveries {
    pub fn around(kept: Arc<dyn Deliveries>, broadcaster: Broadcaster<RefusalChanges>) -> Self {
        Self {
            kept,
            teller: Teller::new(broadcaster),
        }
    }

    fn told(&self, outcome: Result<(), DeliveryError>) -> Result<(), DeliveryError> {
        if outcome.is_ok() {
            self.teller.tell(
                RefusalChanges,
                REFUSALS,
                json!({ "name": "REFUSALS_CHANGED" }),
            );
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

async fn list(State(refusals): State<Refusals>) -> Result<Json<Vec<RefusalDTO>>, Problem> {
    let dead = refusals
        .deliveries
        .dead_letters()
        .await
        .map_err(Problem::unexpected)?;

    let now = refusals.clock.now();

    Ok(Json(
        dead.iter()
            .map(|letter| to_dto(letter, &refusals.dispatcher, now))
            .collect(),
    ))
}

async fn retry(
    State(refusals): State<Refusals>,
    Path(id): Path<i64>,
) -> Result<StatusCode, Problem> {
    let now = refusals.clock.now();
    let dead = refusals
        .deliveries
        .dead_letters()
        .await
        .map_err(Problem::unexpected)?;
    if dead
        .iter()
        .any(|letter| letter.id == id && !letter.may_be_tried_again(now))
    {
        return Err(Problem::refused(
            refused::TOO_LATE_TO_TRY_AGAIN,
            "a change this old is not tried again, as what it was about may be gone for good",
        ));
    }
    refusals
        .deliveries
        .retry(id, now)
        .await
        .map_err(Problem::unexpected)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn acknowledge(
    State(refusals): State<Refusals>,
    Path(id): Path<i64>,
) -> Result<StatusCode, Problem> {
    refusals
        .deliveries
        .acknowledge(id, refusals.clock.now())
        .await
        .map_err(Problem::unexpected)?;

    Ok(StatusCode::NO_CONTENT)
}

fn to_dto(dead: &DeadLetter, dispatcher: &InProcessDispatcher, now: OffsetDateTime) -> RefusalDTO {
    RefusalDTO {
        id: dead.id,
        listener: dead.listener.to_string(),
        routing: dead.message.routing.to_string(),
        attempts: dead.attempts,
        why: dead.why.clone(),
        plainly: dispatcher.when_refused(&dead.listener).map(str::to_owned),
        occurred_at: text::serialize(dead.message.occurred_at),
        given_up_at: text::serialize(dead.given_up_at),
        acknowledged_at: dead.acknowledged_at.map(text::serialize),
        may_be_tried_again: dead.may_be_tried_again(now),
    }
}
