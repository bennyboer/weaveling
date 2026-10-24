use acting::Acting;
use axum::extract::{Path, Query, State};
use axum::http::header::ORIGIN;
use axum::http::{HeaderMap, StatusCode};
use axum::{Extension, Json, Router, routing};
use clock::text;
use email::InvalidEmailAddress;
use eventsourcing::ServiceError;
use invitations_contract::{
    InvitationPageDTO, InvitationSummaryDTO, InviteRequest, decline_reason, refused, status,
};
use invitations_core::{
    DeclineReason, InvitationError, InvitationId, InvitationService, InvitationServiceError,
    ListedInvitation, Status,
};
use serde::Deserialize;
use serving::Problem;

pub fn router(invitations: InvitationService) -> Router {
    Router::new()
        .route("/invitations", routing::post(invite).get(list))
        .route("/invitations/{invitation}", routing::delete(withdraw))
        .with_state(invitations)
}

async fn invite(
    State(invitations): State<InvitationService>,
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

    let summary = invitations
        .issue(&acting, &request.email, origin)
        .await
        .map_err(problem_of)?;

    Ok((
        StatusCode::CREATED,
        Json(to_summary_dto(&ListedInvitation {
            summary,
            status: Status::Open,
        })),
    ))
}

pub const PAGE: usize = 100;

#[derive(Deserialize)]
struct Paging {
    after: Option<String>,
}

async fn list(
    State(invitations): State<InvitationService>,
    Extension(acting): Extension<Acting>,
    Query(paging): Query<Paging>,
) -> Result<Json<InvitationPageDTO>, Problem> {
    let after = paging.after.as_deref().map(invitation_named).transpose()?;
    let mut found = invitations
        .list(&acting, after.as_ref(), PAGE + 1)
        .await
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
    };

    InvitationSummaryDTO {
        id: listed.summary.id.to_string(),
        email: listed.summary.to.to_string(),
        issued_at: text::serialize(listed.summary.issued_at),
        until: text::serialize(listed.summary.until),
        status: told.to_owned(),
        declined_for,
    }
}

fn invitation_named(raw: &str) -> Result<InvitationId, Problem> {
    raw.parse().map_err(|_| {
        Problem::rejected(refused::INVITATION_ID_INVALID, "this is no invitation's id")
    })
}

async fn withdraw(
    State(invitations): State<InvitationService>,
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

fn problem_of(refusal: InvitationServiceError) -> Problem {
    match refusal {
        InvitationServiceError::NotAnOperator => {
            Problem::forbidden(refused::NOT_AN_OPERATOR, refusal)
        }
        InvitationServiceError::InvalidEmail(reason) => match reason {
            InvalidEmailAddress::Malformed => {
                Problem::unprocessable(refused::EMAIL_MALFORMED, reason)
            }
            InvalidEmailAddress::TooLong { max, actual } => {
                Problem::unprocessable(refused::EMAIL_TOO_LONG, reason)
                    .with("max", max)
                    .with("actual", actual)
            }
        },
        InvitationServiceError::Throttled { wait } => {
            Problem::waiting(refused::THROTTLED, refusal, wait)
        }
        InvitationServiceError::Events(ServiceError::NotFound { .. }) => {
            Problem::not_found(refused::INVITATION_NOT_FOUND, "there is no such invitation")
        }
        InvitationServiceError::Events(ServiceError::Refused(InvitationError::AlreadyAccepted)) => {
            Problem::refused(refused::INVITATION_ALREADY_ACCEPTED, refusal)
        }
        unserveable => Problem::unexpected(unserveable),
    }
}
