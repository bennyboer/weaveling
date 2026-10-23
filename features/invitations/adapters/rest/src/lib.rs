use acting::Acting;
use axum::extract::{Path, State};
use axum::http::header::ORIGIN;
use axum::http::{HeaderMap, StatusCode};
use axum::{Extension, Json, Router, routing};
use email::InvalidEmailAddress;
use eventsourcing::ServiceError;
use invitations_contract::{InviteRequest, InvitedDTO, refused};
use invitations_core::{InvitationError, InvitationId, InvitationService, InvitationServiceError};
use serving::Problem;

pub fn router(invitations: InvitationService) -> Router {
    Router::new()
        .route("/invitations", routing::post(invite))
        .route("/invitations/{invitation}", routing::delete(withdraw))
        .with_state(invitations)
}

async fn invite(
    State(invitations): State<InvitationService>,
    Extension(acting): Extension<Acting>,
    headers: HeaderMap,
    Json(request): Json<InviteRequest>,
) -> Result<(StatusCode, Json<InvitedDTO>), Problem> {
    let origin = headers
        .get(ORIGIN)
        .and_then(|origin| origin.to_str().ok())
        .ok_or_else(|| {
            Problem::rejected(
                refused::ORIGIN_MISSING,
                "an invitation's link is made from the page it was sent from",
            )
        })?;

    let id = invitations
        .issue(&acting, &request.email, origin)
        .await
        .map_err(problem_of)?;

    Ok((StatusCode::CREATED, Json(InvitedDTO { id: id.to_string() })))
}

async fn withdraw(
    State(invitations): State<InvitationService>,
    Extension(acting): Extension<Acting>,
    Path(invitation): Path<String>,
) -> Result<StatusCode, Problem> {
    let invitation: InvitationId = invitation.parse().map_err(|_| {
        Problem::rejected(refused::INVITATION_ID_INVALID, "this is no invitation's id")
    })?;

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
