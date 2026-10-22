use accounts_contract::{
    NewPasswordRequest, PasswordResetRequest, SESSION_COOKIE, SendCodeRequest, SignInRequest,
    SignUpRequest, SignedInDTO, VerifyEmailRequest, refused, stage,
};
use accounts_core::{
    AccountService, AccountServiceError, InvalidAccountName, InvalidEmailAddress, InvalidPassword,
    Session, SessionToken, SignedIn, Stage,
};
use axum::Json;
use axum::Router;
use axum::extract::{Request, State};
use axum::http::header::{COOKIE, SET_COOKIE};
use axum::http::{HeaderMap, StatusCode};
use axum::middleware::{Next, from_fn_with_state};
use axum::response::{IntoResponse, Response};
use axum::routing::{get, post};
use serving::Problem;
use time::Duration;

pub fn router(accounts: AccountService) -> Router {
    Router::new()
        .route("/accounts", post(sign_up))
        .route("/accounts/verification", post(verify))
        .route("/accounts/verification/code", post(send_code_again))
        .route("/accounts/password-reset", post(ask_to_reset_password))
        .route("/accounts/password", post(reset_password))
        .route("/sessions", post(begin_session))
        .route("/sessions/current", get(current).delete(end_session))
        .with_state(accounts)
}

pub fn admitting_only_the_signed_in(accounts: AccountService, routes: Router) -> Router {
    routes.layer(from_fn_with_state(accounts, admit))
}

async fn admit(
    State(accounts): State<AccountService>,
    mut request: Request,
    next: Next,
) -> Response {
    let Some(token) = presented(request.headers()) else {
        return not_signed_in().into_response();
    };

    match accounts.signed_in(&token).await {
        Ok(Some(signed_in)) if signed_in.session.may_act() => {
            request
                .extensions_mut()
                .insert(signed_in.session.account.acting());
            next.run(request).await
        }
        Ok(_) => not_signed_in().into_response(),
        Err(failed) => problem_of(failed).into_response(),
    }
}

async fn sign_up(
    State(accounts): State<AccountService>,
    Json(request): Json<SignUpRequest>,
) -> Result<StatusCode, Problem> {
    accounts
        .sign_up(&request.email, &request.name, &request.password)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::ACCEPTED)
}

async fn verify(
    State(accounts): State<AccountService>,
    Json(request): Json<VerifyEmailRequest>,
) -> Result<StatusCode, Problem> {
    accounts
        .verify(&request.email, &request.code)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn send_code_again(
    State(accounts): State<AccountService>,
    Json(request): Json<SendCodeRequest>,
) -> Result<StatusCode, Problem> {
    accounts
        .send_code_again(&request.email)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::ACCEPTED)
}

async fn ask_to_reset_password(
    State(accounts): State<AccountService>,
    Json(request): Json<PasswordResetRequest>,
) -> Result<StatusCode, Problem> {
    accounts
        .ask_to_reset_password(&request.email)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::ACCEPTED)
}

async fn reset_password(
    State(accounts): State<AccountService>,
    Json(request): Json<NewPasswordRequest>,
) -> Result<StatusCode, Problem> {
    accounts
        .reset_password(&request.email, &request.code, &request.password)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn begin_session(
    State(accounts): State<AccountService>,
    Json(request): Json<SignInRequest>,
) -> Result<Response, Problem> {
    let (token, signed_in) = accounts
        .begin_session(&request.email, &request.password)
        .await
        .map_err(problem_of)?;

    Ok(([(SET_COOKIE, holding(&token))], Json(to_dto(&signed_in))).into_response())
}

async fn current(
    State(accounts): State<AccountService>,
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
    State(accounts): State<AccountService>,
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

fn problem_of(refusal: AccountServiceError) -> Problem {
    match refusal {
        AccountServiceError::InvalidEmail(reason) => match reason {
            InvalidEmailAddress::Malformed => {
                Problem::unprocessable(refused::EMAIL_MALFORMED, reason)
            }
            InvalidEmailAddress::TooLong { max, actual } => {
                Problem::unprocessable(refused::EMAIL_TOO_LONG, reason)
                    .with("max", max)
                    .with("actual", actual)
            }
        },
        AccountServiceError::InvalidName(reason) => match reason {
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
        AccountServiceError::InvalidPassword(reason) => match reason {
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
        AccountServiceError::CodeWrong => Problem::unprocessable(refused::CODE_WRONG, refusal),
        AccountServiceError::CodeExpired => Problem::unprocessable(refused::CODE_EXPIRED, refusal),
        AccountServiceError::CodeExhausted => {
            Problem::unprocessable(refused::CODE_EXHAUSTED, refusal)
        }
        AccountServiceError::SignInRefused => {
            Problem::unauthorized(refused::SIGN_IN_REFUSED, refusal)
        }
        AccountServiceError::NotVerifiedYet => Problem::forbidden(refused::NOT_VERIFIED, refusal),
        AccountServiceError::Throttled { wait } => {
            let seconds = whole_seconds_rounded_up(wait);
            Problem::throttled(refused::THROTTLED, refusal, seconds)
                .with("minutes", seconds.div_ceil(60))
        }
        unserveable => Problem::unexpected(unserveable),
    }
}

fn whole_seconds_rounded_up(wait: Duration) -> u64 {
    let whole = wait.whole_seconds().max(0).unsigned_abs();
    if wait.subsec_nanoseconds() > 0 {
        whole + 1
    } else {
        whole
    }
}
