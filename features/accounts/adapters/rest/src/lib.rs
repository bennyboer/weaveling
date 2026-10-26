use accounts_contract::{
    AccountPageDTO, AccountSummaryDTO, BecomeOperatorRequest, LockDTO, LockRequest,
    NewPasswordRequest, PasswordResetRequest, SESSION_COOKIE, SendCodeRequest, SignInRequest,
    SignUpByInvitationRequest, SignUpRequest, SignedInDTO, TakeUpInvitationRequest,
    VerifyEmailRequest, lock_kind, refused, stage,
};
use accounts_core::{
    AccountError, AccountId, AccountSummary, AccountsCore, AccountsError, InvalidAccountName,
    InvalidEmailAddress, InvalidLockReason, InvalidPassword, LockKind, Session, SessionToken,
    SignedIn, Stage,
};
use acting::Acting;
use axum::Router;
use axum::extract::{Path, Query, Request, State};
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware;
use axum::middleware::Next;
use axum::response::{IntoResponse, Response};
use axum::routing;
use axum::{Extension, Json};
use clock::text;
use eventsourcing::ServiceError;
use serde::Deserialize;
use serving::Problem;

pub fn router(accounts: AccountsCore) -> Router {
    Router::new()
        .route("/accounts", routing::post(sign_up))
        .route(
            "/accounts/by-invitation",
            routing::post(sign_up_by_invitation),
        )
        .route("/accounts/verification", routing::post(verify))
        .route(
            "/accounts/verification/code",
            routing::post(send_code_again),
        )
        .route(
            "/accounts/password-reset",
            routing::post(ask_to_reset_password),
        )
        .route("/accounts/password", routing::post(reset_password))
        .route("/sessions", routing::post(begin_session))
        .route(
            "/sessions/current",
            routing::get(current).delete(end_session),
        )
        .with_state(accounts)
}

pub fn guarded_router(accounts: AccountsCore) -> Router {
    Router::new()
        .route("/operators", routing::post(become_operator))
        .route(
            "/accounts/current/invitations",
            routing::post(take_up_invitation),
        )
        .route("/instance/accounts", routing::get(list_accounts))
        .route(
            "/instance/accounts/{account}/lock",
            routing::put(lock).delete(unlock),
        )
        .route(
            "/instance/accounts/{account}/operator",
            routing::put(appoint_operator).delete(dismiss_operator),
        )
        .with_state(accounts)
}

pub fn admitting_only_the_signed_in(accounts: AccountsCore, routes: Router) -> Router {
    routes.layer(middleware::from_fn_with_state(accounts, admit))
}

async fn admit(State(accounts): State<AccountsCore>, mut request: Request, next: Next) -> Response {
    let Some(token) = presented(request.headers()) else {
        return not_signed_in().into_response();
    };

    match accounts.signed_in(&token).await {
        Ok(Some(signed_in)) if signed_in.session.may_act() => {
            let account = signed_in.session.account;
            request.extensions_mut().insert(Acting {
                agent: account.acting(),
                operator: signed_in.account.is_operator(),
            });
            request.extensions_mut().insert(account.acting());
            request.extensions_mut().insert(account);
            next.run(request).await
        }
        Ok(_) => not_signed_in().into_response(),
        Err(failed) => problem_of(failed).into_response(),
    }
}

async fn sign_up(
    State(accounts): State<AccountsCore>,
    Json(request): Json<SignUpRequest>,
) -> Result<StatusCode, Problem> {
    match &request.operator_code {
        Some(code) => {
            accounts
                .sign_up_to_operate(&request.email, &request.name, &request.password, code)
                .await
        }
        None => {
            accounts
                .sign_up(&request.email, &request.name, &request.password)
                .await
        }
    }
    .map_err(problem_of)?;

    Ok(StatusCode::ACCEPTED)
}

async fn sign_up_by_invitation(
    State(accounts): State<AccountsCore>,
    Json(request): Json<SignUpByInvitationRequest>,
) -> Result<Response, Problem> {
    let (token, signed_in) = accounts
        .sign_up_by_invitation(&request.token, &request.name, &request.password)
        .await
        .map_err(problem_of)?;

    Ok((
        StatusCode::CREATED,
        [(SET_COOKIE, holding(&token))],
        Json(to_dto(&signed_in)),
    )
        .into_response())
}

async fn verify(
    State(accounts): State<AccountsCore>,
    Json(request): Json<VerifyEmailRequest>,
) -> Result<StatusCode, Problem> {
    accounts
        .verify(&request.email, &request.code)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn send_code_again(
    State(accounts): State<AccountsCore>,
    Json(request): Json<SendCodeRequest>,
) -> Result<StatusCode, Problem> {
    accounts
        .send_code_again(&request.email)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::ACCEPTED)
}

async fn ask_to_reset_password(
    State(accounts): State<AccountsCore>,
    Json(request): Json<PasswordResetRequest>,
) -> Result<StatusCode, Problem> {
    accounts
        .ask_to_reset_password(&request.email)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::ACCEPTED)
}

async fn reset_password(
    State(accounts): State<AccountsCore>,
    Json(request): Json<NewPasswordRequest>,
) -> Result<StatusCode, Problem> {
    accounts
        .reset_password(&request.email, &request.code, &request.password)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn begin_session(
    State(accounts): State<AccountsCore>,
    Json(request): Json<SignInRequest>,
) -> Result<Response, Problem> {
    let (token, signed_in) = accounts
        .begin_session(&request.email, &request.password)
        .await
        .map_err(problem_of)?;

    Ok(([(SET_COOKIE, holding(&token))], Json(to_dto(&signed_in))).into_response())
}

async fn take_up_invitation(
    State(accounts): State<AccountsCore>,
    Extension(account): Extension<AccountId>,
    Json(request): Json<TakeUpInvitationRequest>,
) -> Result<StatusCode, Problem> {
    accounts
        .take_up_invitation(&account, &request.token)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn become_operator(
    State(accounts): State<AccountsCore>,
    Extension(account): Extension<AccountId>,
    Json(request): Json<BecomeOperatorRequest>,
) -> Result<StatusCode, Problem> {
    accounts
        .become_operator(&account, &request.code)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

pub const PAGE: usize = 100;

#[derive(Deserialize)]
struct Paging {
    after: Option<String>,
}

async fn list_accounts(
    State(accounts): State<AccountsCore>,
    Extension(by): Extension<AccountId>,
    Query(paging): Query<Paging>,
) -> Result<Json<AccountPageDTO>, Problem> {
    let after = paging.after.as_deref().map(account_named).transpose()?;
    let mut found = accounts
        .list(&by, after.as_ref(), PAGE + 1)
        .await
        .map_err(problem_of)?;
    let more = found.len() > PAGE;
    found.truncate(PAGE);

    Ok(Json(AccountPageDTO {
        next: more
            .then(|| found.last().map(|last| last.id.to_string()))
            .flatten(),
        accounts: found.iter().map(to_summary_dto).collect(),
    }))
}

async fn lock(
    State(accounts): State<AccountsCore>,
    Extension(by): Extension<AccountId>,
    Path(account): Path<String>,
    Json(request): Json<LockRequest>,
) -> Result<StatusCode, Problem> {
    accounts
        .lock(&by, &account_named(&account)?, &request.reason)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn unlock(
    State(accounts): State<AccountsCore>,
    Extension(by): Extension<AccountId>,
    Path(account): Path<String>,
) -> Result<StatusCode, Problem> {
    accounts
        .unlock(&by, &account_named(&account)?)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn appoint_operator(
    State(accounts): State<AccountsCore>,
    Extension(by): Extension<AccountId>,
    Path(account): Path<String>,
) -> Result<StatusCode, Problem> {
    accounts
        .appoint_operator(&by, &account_named(&account)?)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn dismiss_operator(
    State(accounts): State<AccountsCore>,
    Extension(by): Extension<AccountId>,
    Path(account): Path<String>,
) -> Result<StatusCode, Problem> {
    accounts
        .dismiss_operator(&by, &account_named(&account)?)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

fn account_named(given: &str) -> Result<AccountId, Problem> {
    given
        .parse()
        .map_err(|_| Problem::rejected(refused::ACCOUNT_ID_INVALID, "this is no account's id"))
}

fn to_summary_dto(summary: &AccountSummary) -> AccountSummaryDTO {
    AccountSummaryDTO {
        id: summary.id.to_string(),
        name: summary.name.to_string(),
        email: summary.email.to_string(),
        verified: summary.verified,
        operator: summary.operator,
        signed_up_at: text::serialize(summary.signed_up_at),
        locks: summary
            .locks
            .iter()
            .map(|(kind, lock)| LockDTO {
                kind: match kind {
                    LockKind::Operator => lock_kind::OPERATOR,
                }
                .to_owned(),
                reason: lock.reason.to_string(),
                since: text::serialize(lock.since),
            })
            .collect(),
    }
}

async fn current(
    State(accounts): State<AccountsCore>,
    headers: HeaderMap,
) -> Result<Json<SignedInDTO>, Problem> {
    let Some(token) = presented(&headers) else {
        return Err(not_signed_in());
    };

    accounts
        .signed_in(&token)
        .await
        .map_err(problem_of)?
        .map(|signed_in| Json(to_dto(&signed_in)))
        .ok_or_else(not_signed_in)
}

async fn end_session(
    State(accounts): State<AccountsCore>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    if let Some(token) = presented(&headers) {
        accounts.end_session(&token).await.map_err(problem_of)?;
    }

    Ok(([(SET_COOKIE, letting_go())], StatusCode::NO_CONTENT).into_response())
}

fn to_dto(signed_in: &SignedIn) -> SignedInDTO {
    SignedInDTO {
        account: signed_in.session.account.to_string(),
        name: signed_in.account.name().to_string(),
        email: signed_in.account.email().to_string(),
        stage: match signed_in.session.stage {
            Stage::SignedIn => stage::SIGNED_IN,
            Stage::SecondFactorOwed => stage::SECOND_FACTOR_OWED,
        }
        .to_owned(),
        operator: signed_in.account.is_operator(),
    }
}

fn holding(token: &SessionToken) -> String {
    format!(
        "{SESSION_COOKIE}={}; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age={}",
        token.as_str(),
        Session::LASTS.whole_seconds()
    )
}

fn letting_go() -> String {
    format!("{SESSION_COOKIE}=; HttpOnly; Secure; SameSite=Strict; Path=/; Max-Age=0")
}

fn presented(headers: &HeaderMap) -> Option<SessionToken> {
    headers
        .get_all(COOKIE)
        .iter()
        .filter_map(|sent| sent.to_str().ok())
        .flat_map(|sent| sent.split(';'))
        .filter_map(|pair| pair.trim().split_once('='))
        .find(|(name, _)| *name == SESSION_COOKIE)
        .map(|(_, token)| SessionToken::presented(token))
}

fn not_signed_in() -> Problem {
    Problem::unauthorized(refused::NOT_SIGNED_IN, "sign in first")
}

fn problem_of(refusal: AccountsError) -> Problem {
    match refusal {
        AccountsError::InvalidEmail(reason) => match reason {
            InvalidEmailAddress::Malformed => {
                Problem::unprocessable(refused::EMAIL_MALFORMED, reason)
            }
            InvalidEmailAddress::TooLong { max, actual } => {
                Problem::unprocessable(refused::EMAIL_TOO_LONG, reason)
                    .with("max", max)
                    .with("actual", actual)
            }
        },
        AccountsError::InvalidName(reason) => match reason {
            InvalidAccountName::Blank => Problem::unprocessable(refused::NAME_BLANK, reason),
            InvalidAccountName::ControlCharacter => {
                Problem::unprocessable(refused::NAME_CONTROL_CHARACTER, reason)
            }
            InvalidAccountName::TooLong { max, actual } => {
                Problem::unprocessable(refused::NAME_TOO_LONG, reason)
                    .with("max", max)
                    .with("actual", actual)
            }
        },
        AccountsError::InvalidPassword(reason) => match reason {
            InvalidPassword::TooShort { min, actual } => {
                Problem::unprocessable(refused::PASSWORD_TOO_SHORT, reason)
                    .with("min", min)
                    .with("actual", actual)
            }
            InvalidPassword::TooLong { max, actual } => {
                Problem::unprocessable(refused::PASSWORD_TOO_LONG, reason)
                    .with("max", max)
                    .with("actual", actual)
            }
        },
        AccountsError::CodeWrong => Problem::unprocessable(refused::CODE_WRONG, refusal),
        AccountsError::CodeExpired => Problem::unprocessable(refused::CODE_EXPIRED, refusal),
        AccountsError::CodeExhausted => Problem::unprocessable(refused::CODE_EXHAUSTED, refusal),
        AccountsError::SignInRefused => Problem::unauthorized(refused::SIGN_IN_REFUSED, refusal),
        AccountsError::NotVerifiedYet => Problem::forbidden(refused::NOT_VERIFIED, refusal),
        AccountsError::Locked => Problem::forbidden(refused::ACCOUNT_LOCKED, refusal),
        AccountsError::NotAnOperator => Problem::forbidden(refused::NOT_AN_OPERATOR, refusal),
        AccountsError::LastOperator => Problem::refused(refused::LAST_OPERATOR, refusal),
        AccountsError::Events(ServiceError::Refused(AccountError::NotVerifiedYet)) => {
            Problem::refused(refused::OPERATOR_ADDRESS_NOT_VERIFIED, refusal)
        }
        AccountsError::Events(ServiceError::Refused(AccountError::LockedCannotBecomeOperator)) => {
            Problem::refused(refused::LOCKED_CANNOT_BECOME_OPERATOR, refusal)
        }
        AccountsError::InvalidLockReason(reason) => match reason {
            InvalidLockReason::Blank => Problem::unprocessable(refused::LOCK_REASON_BLANK, reason),
            InvalidLockReason::ControlCharacter => {
                Problem::unprocessable(refused::LOCK_REASON_CONTROL_CHARACTER, reason)
            }
            InvalidLockReason::TooLong { max, actual } => {
                Problem::unprocessable(refused::LOCK_REASON_TOO_LONG, reason)
                    .with("max", max)
                    .with("actual", actual)
            }
        },
        AccountsError::Events(ServiceError::Refused(AccountError::OperatorCannotBeLocked)) => {
            Problem::refused(refused::OPERATOR_CANNOT_BE_LOCKED, refusal)
        }
        AccountsError::Events(ServiceError::NotFound { .. }) => {
            Problem::not_found(refused::ACCOUNT_NOT_FOUND, "there is no such account")
        }
        AccountsError::OperatorCodeWrong => {
            Problem::unprocessable(refused::OPERATOR_CODE_WRONG, refusal)
        }
        AccountsError::Throttled { wait } => Problem::waiting(refused::THROTTLED, refusal, wait),
        AccountsError::SigningUpByInvitationOnly => {
            Problem::forbidden(refused::BY_INVITATION_ONLY, refusal)
        }
        AccountsError::InvitationInvalid => {
            Problem::unprocessable(refused::INVITATION_INVALID, refusal)
        }
        AccountsError::InvitationExpired => {
            Problem::unprocessable(refused::INVITATION_EXPIRED, refusal)
        }
        AccountsError::InvitationForAnotherAddress { ref to } => {
            let to = to.to_string();
            Problem::forbidden(refused::INVITATION_FOR_ANOTHER_ADDRESS, refusal).with("to", to)
        }
        AccountsError::InvitationNotByAnOperator => {
            Problem::forbidden(refused::INVITATION_NOT_BY_AN_OPERATOR, refusal)
        }
        AccountsError::EmailAddressHasAccount => {
            Problem::refused(refused::EMAIL_ADDRESS_HAS_ACCOUNT, refusal)
        }
        unserveable => Problem::unexpected(unserveable),
    }
}
