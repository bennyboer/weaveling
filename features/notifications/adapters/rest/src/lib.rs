mod telling;
mod topic;

use std::collections::BTreeMap;

use axum::extract::{Path, State};
use axum::http::StatusCode;
use axum::routing;
use axum::{Extension, Json, Router};
use clock::text;
use eventsourcing::{AccountId, Agent, ServiceError};
use notifications_contract::{AboutDTO, NoticeDTO, role};
use notifications_core::{
    About, Names, NoticeSummary, NotificationsCore, NotificationsError, Role,
};
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
        open.iter().map(|notice| to_dto(notice, &names)).collect(),
    ))
}

async fn dismiss(
    State(notifications): State<NotificationsCore>,
    Extension(agent): Extension<Agent>,
    Path(notice): Path<String>,
) -> Result<StatusCode, Problem> {
    notifications
        .dismiss(&notice, &agent)
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
        beyond_reach => Problem::unexpected(beyond_reach),
    }
}

fn to_dto(summary: &NoticeSummary, names: &Names) -> NoticeDTO {
    NoticeDTO {
        names: named_in(&summary.about, names),
        id: summary.id.to_string(),
        noted_at: text::serialize(summary.noted_at),
        about: match &summary.about {
            About::RoleChanged { project, role, by } => AboutDTO::RoleChanged {
                project: project.to_string(),
                role: named(*role).to_owned(),
                by: by.as_ref().map(ToString::to_string),
            },
        },
    }
}

fn named_in(about: &About, names: &Names) -> BTreeMap<String, String> {
    let project = about.project().and_then(|project| {
        names
            .projects
            .get(project)
            .map(|name| (project.to_string(), name.clone()))
    });
    let people = about.accounts().into_iter().filter_map(|account| {
        names
            .people
            .get(account)
            .map(|name| (account.to_string(), name.clone()))
    });

    project.into_iter().chain(people).collect()
}

fn named(given: Role) -> &'static str {
    match given {
        Role::Owner => role::OWNER,
        Role::Author => role::AUTHOR,
        Role::Reader => role::READER,
    }
}
