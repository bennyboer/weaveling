mod telling;
mod topic;
mod transformer;

use axum::extract::{Path, State};
use axum::http::{HeaderMap, StatusCode};
use axum::routing;
use axum::{Extension, Json, Router};
use eventsourcing::{AccountId, Agent, ServiceError, StoreError};
use notifications_contract::NoticeDTO;
use notifications_core::{NotificationsCore, NotificationsError};
use serving::Problem;

pub use telling::TellingNotices;
pub use topic::NotificationsTopic;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct NoticeChanges(pub AccountId);

pub fn router(notifications: NotificationsCore) -> Router {
    Router::new()
        .route("/notifications", routing::get(open))
        .route("/notifications/{notice}/dismissed", routing::post(dismiss))
        .with_state(notifications)
}

async fn open(
    State(notifications): State<NotificationsCore>,
    Extension(agent): Extension<Agent>,
) -> Result<Json<Vec<NoticeDTO>>, Problem> {
    let open = notifications.open_for(&agent).await.map_err(problem_of)?;
    let names = notifications.names_in(&open).await.map_err(problem_of)?;

    Ok(Json(
        open.iter()
            .map(|notice| transformer::to_dto(notice, &names))
            .collect(),
    ))
}

async fn dismiss(
    State(notifications): State<NotificationsCore>,
    Extension(agent): Extension<Agent>,
    Path(notice): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    notifications
        .dismiss(&notice, serving::demanded(&headers)?, &agent)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

fn problem_of(failure: NotificationsError) -> Problem {
    match failure {
        NotificationsError::InvalidId(_)
        | NotificationsError::Events(ServiceError::NotFound { .. }) => {
            Problem::not_found(serving::NOT_FOUND, "there is no such notice")
        }
        NotificationsError::Events(ServiceError::Store(StoreError::Outdated {
            ref kind, ..
        })) => Problem::outdated(kind),
        beyond_reach => Problem::unexpected(beyond_reach),
    }
}
