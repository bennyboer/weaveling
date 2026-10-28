mod telling;

use acting::Acting;
use axum::extract::ws::WebSocketUpgrade;
use axum::extract::{Path, Query, State};
use axum::http::header::ORIGIN;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};
use axum::{Extension, Json, Router, routing};
use clock::text;
use email::InvalidEmailAddress;
use eventsourcing::ServiceError;
use invitations_contract::{
    InvitationPageDTO, InvitationSummaryDTO, InvitationToYouDTO, InviteRequest, ScopeDTO,
    decline_reason, refused, role, status,
};
use invitations_core::{
    DeclineReason, InvitationError, InvitationId, InvitationToYou, InvitationsCore,
    InvitationsError, ListedInvitation, Role, Scope, Status,
};
use live::Broadcaster;
use serde::Deserialize;
use serving::Problem;

pub use telling::TellingInvitations;

pub fn router(invitations: InvitationsCore) -> Router {
    Router::new()
        .route("/invitations", routing::post(invite).get(list))
        .route("/invitations/mine", routing::get(mine))
        .route("/invitations/{invitation}", routing::delete(withdraw))
        .with_state(invitations)
}

#[derive(Clone)]
struct Following {
    invitations: InvitationsCore,
    broadcaster: Broadcaster,
}

pub fn live(invitations: InvitationsCore, broadcaster: Broadcaster) -> Router {
    Router::new()
        .route("/invitations/mine/live", routing::get(follow_mine))
        .with_state(Following {
            invitations,
            broadcaster,
        })
}

async fn follow_mine(
    upgrade: WebSocketUpgrade,
    State(following): State<Following>,
    Extension(acting): Extension<Acting>,
) -> Response {
    match following.invitations.address_of(&acting).await {
        Ok(Some(address)) => live::streaming(upgrade, following.broadcaster.hear(address.as_str())),
        Ok(None) => StatusCode::NOT_FOUND.into_response(),
        Err(_) => StatusCode::INTERNAL_SERVER_ERROR.into_response(),
    }
}

async fn invite(
    State(invitations): State<InvitationsCore>,
    Extension(acting): Extension<Acting>,
    headers: HeaderMap,
    Json(request): Json<InviteRequest>,
) -> Result<(StatusCode, Json<InvitationSummaryDTO>), Problem> {
    let origin = headers
        .get(ORIGIN)
        .and_then(|origin| origin.to_str().ok())
        .ok_or_else(|| {
            Problem::rejected(
                refused::ORIGIN_MISSING,
                "an invitation's link is made from the page it was sent from",
            )
        })?;

    let summary = match &request.scope {
        ScopeDTO::Instance => {
            invitations
                .invite_to_instance(&acting, &request.email, origin)
                .await
        }
        ScopeDTO::Project { project, role } => {
            let as_whom = to_role(role).ok_or_else(|| {
                Problem::unprocessable(
                    refused::ROLE_UNKNOWN,
                    "an invitation into a project invites as an owner, an author or a reader",
                )
            })?;

            invitations
                .invite_to_project(&acting, project, as_whom, &request.email, origin)
                .await
        }
    }
    .map_err(problem_of)?;

    Ok((
        StatusCode::CREATED,
        Json(to_summary_dto(&ListedInvitation {
            summary,
            status: Status::Open,
        })),
    ))
}

async fn mine(
    State(invitations): State<InvitationsCore>,
    Extension(acting): Extension<Acting>,
) -> Result<Json<Vec<InvitationToYouDTO>>, Problem> {
    let yours = invitations
        .invitations_to(&acting)
        .await
        .map_err(problem_of)?;

    Ok(Json(yours.iter().map(to_you_dto).collect()))
}

fn to_you_dto(yours: &InvitationToYou) -> InvitationToYouDTO {
    InvitationToYouDTO {
        id: yours.id.to_string(),
        project: yours.project.to_string(),
        project_name: yours.project_name.clone(),
        role: named(yours.role).to_owned(),
        invited_by: yours.invited_by.clone(),
        until: text::serialize(yours.until),
    }
}

pub const PAGE: usize = 100;

#[derive(Deserialize)]
struct Paging {
    project: Option<String>,
    after: Option<String>,
}

async fn list(
    State(invitations): State<InvitationsCore>,
    Extension(acting): Extension<Acting>,
    Query(paging): Query<Paging>,
) -> Result<Json<InvitationPageDTO>, Problem> {
    let after = paging.after.as_deref().map(invitation_named).transpose()?;
    let mut found = match paging.project.as_deref() {
        None => {
            invitations
                .instance_invitations(&acting, after.as_ref(), PAGE + 1)
                .await
        }
        Some(project) => {
            invitations
                .project_invitations(&acting, project, after.as_ref(), PAGE + 1)
                .await
        }
    }
    .map_err(problem_of)?;
    let more = found.len() > PAGE;
    found.truncate(PAGE);

    Ok(Json(InvitationPageDTO {
        next: more
            .then(|| found.last().map(|last| last.summary.id.to_string()))
            .flatten(),
        invitations: found.iter().map(to_summary_dto).collect(),
    }))
}

fn to_summary_dto(listed: &ListedInvitation) -> InvitationSummaryDTO {
    let (told, declined_for) = match listed.status {
        Status::Open => (status::OPEN, None),
        Status::Expired => (status::EXPIRED, None),
        Status::Declined(DeclineReason::AddressHasAccount) => (
            status::DECLINED,
            Some(decline_reason::ADDRESS_HAS_ACCOUNT.to_owned()),
        ),
        Status::Declined(DeclineReason::TurnedDown) => (
            status::DECLINED,
            Some(decline_reason::TURNED_DOWN.to_owned()),
        ),
    };

    InvitationSummaryDTO {
        id: listed.summary.id.to_string(),
        email: listed.summary.to.to_string(),
        issued_at: text::serialize(listed.summary.issued_at),
        until: text::serialize(listed.summary.until),
        status: told.to_owned(),
        declined_for,
        scope: match &listed.summary.scope {
            Scope::Instance => ScopeDTO::Instance,
            Scope::Project { project, role } => ScopeDTO::Project {
                project: project.to_string(),
                role: named(*role).to_owned(),
            },
        },
    }
}

fn named(given: Role) -> &'static str {
    match given {
        Role::Owner => role::OWNER,
        Role::Author => role::AUTHOR,
        Role::Reader => role::READER,
    }
}

fn to_role(named: &str) -> Option<Role> {
    match named {
        role::OWNER => Some(Role::Owner),
        role::AUTHOR => Some(Role::Author),
        role::READER => Some(Role::Reader),
        _ => None,
    }
}

fn invitation_named(raw: &str) -> Result<InvitationId, Problem> {
    raw.parse().map_err(|_| {
        Problem::rejected(refused::INVITATION_ID_INVALID, "this is no invitation's id")
    })
}

async fn withdraw(
    State(invitations): State<InvitationsCore>,
    Extension(acting): Extension<Acting>,
    Path(invitation): Path<String>,
) -> Result<StatusCode, Problem> {
    let invitation = invitation_named(&invitation)?;

    invitations
        .withdraw(&acting, &invitation)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

fn problem_of(refusal: InvitationsError) -> Problem {
    match refusal {
        InvitationsError::NotAnOperator => Problem::forbidden(refused::NOT_AN_OPERATOR, refusal),
        InvitationsError::NotAnOwner => Problem::forbidden(refused::NOT_AN_OWNER, refusal),
        InvitationsError::ProjectNotThere => Problem::not_found(serving::NOT_FOUND, refusal),
        InvitationsError::InvalidEmail(reason) => match reason {
            InvalidEmailAddress::Malformed => {
                Problem::unprocessable(refused::EMAIL_MALFORMED, reason)
            }
            InvalidEmailAddress::TooLong { max, actual } => {
                Problem::unprocessable(refused::EMAIL_TOO_LONG, reason)
                    .with("max", max)
                    .with("actual", actual)
            }
        },
        InvitationsError::Throttled { wait } => Problem::waiting(refused::THROTTLED, refusal, wait),
        InvitationsError::Events(ServiceError::NotFound { .. }) => {
            Problem::not_found(refused::INVITATION_NOT_FOUND, "there is no such invitation")
        }
        InvitationsError::Events(ServiceError::Refused(InvitationError::AlreadyAccepted)) => {
            Problem::refused(refused::INVITATION_ALREADY_ACCEPTED, refusal)
        }
        unserveable => Problem::unexpected(unserveable),
    }
}
