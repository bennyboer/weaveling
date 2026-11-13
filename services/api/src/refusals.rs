use std::sync::Arc;

use accounts_contract::refused as accounts_refused;
use accounts_core::AccountsCore;
use async_trait::async_trait;
use axum::Extension;
use axum::Json;
use axum::Router;
use axum::extract::{Path, Request, State};
use axum::http::StatusCode;
use axum::middleware::{self, Next};
use axum::response::{IntoResponse, Response};
use axum::routing;
use clock::Clock;
use clock::text;
use eventsourcing::Agent;
use live::{About, Broadcaster, Refused, Subscribed, Teller, Topic};
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

/// The refusals, followed live.
/// In local mode anyone may follow them; when there are accounts, only an operator may.
pub struct RefusalsTopic {
    pub broadcaster: Broadcaster<RefusalChanges>,
    pub operators: Option<AccountsCore>,
}

#[async_trait]
impl Topic for RefusalsTopic {
    fn named(&self) -> &'static str {
        REFUSALS
    }

    async fn subscribe(&self, agent: &Agent, _about: &About) -> Result<Subscribed, Refused> {
        if let Some(operators) = &self.operators
            && !operators
                .operates(agent)
                .await
                .map_err(|_| Refused::Failed)?
        {
            return Err(Refused::NotThere);
        }

        Ok(Subscribed::hearing(self.broadcaster.hear(RefusalChanges)))
    }
}

/// Lets only an operator reach the refusals, when there are accounts.
/// The refusals show what went wrong inside, and acknowledging or retrying them is the operator's job.
pub fn for_operators(refusals: Router, accounts: AccountsCore) -> Router {
    refusals.route_layer(middleware::from_fn_with_state(accounts, operators_only))
}

async fn operators_only(
    State(accounts): State<AccountsCore>,
    Extension(agent): Extension<Agent>,
    request: Request,
    next: Next,
) -> Response {
    match accounts.operates(&agent).await {
        Ok(true) => next.run(request).await,
        Ok(false) => Problem::forbidden(
            accounts_refused::NOT_AN_OPERATOR,
            "only an operator sees what was refused",
        )
        .into_response(),
        Err(failed) => Problem::unexpected(failed).into_response(),
    }
}

pub fn router(
    deliveries: Arc<dyn Deliveries>,
    dispatcher: Arc<InProcessDispatcher>,
    clock: Arc<dyn Clock>,
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

    async fn mark_as_handled(&self, queued: &Queued) -> Result<(), DeliveryError> {
        self.kept.mark_as_handled(queued).await
    }

    async fn mark_as_refused(
        &self,
        queued: &Queued,
        why: &str,
        again_at: OffsetDateTime,
    ) -> Result<(), DeliveryError> {
        self.kept.mark_as_refused(queued, why, again_at).await
    }

    async fn give_up(
        &self,
        queued: &Queued,
        why: &str,
        at: OffsetDateTime,
    ) -> Result<(), DeliveryError> {
        self.told(self.kept.give_up(queued, why, at).await)
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

    async fn forget_old_dead_letters(
        &self,
        before: OffsetDateTime,
        at_most: i64,
    ) -> Result<u64, DeliveryError> {
        self.kept.forget_old_dead_letters(before, at_most).await
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
