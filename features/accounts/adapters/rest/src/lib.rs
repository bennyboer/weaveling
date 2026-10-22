use accounts_contract::{SendCodeRequest, SignUpRequest, VerifyEmailRequest, refused};
use accounts_core::{
    AccountService, AccountServiceError, InvalidAccountName, InvalidEmailAddress, InvalidPassword,
};
use axum::Json;
use axum::Router;
use axum::extract::State;
use axum::http::StatusCode;
use axum::routing::post;
use serving::Problem;

pub fn router(accounts: AccountService) -> Router {
    Router::new()
        .route("/accounts", post(sign_up))
        .route("/accounts/verification", post(verify))
        .route("/accounts/verification/code", post(send_code_again))
        .with_state(accounts)
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
        unserveable => Problem::unexpected(unserveable),
    }
}
