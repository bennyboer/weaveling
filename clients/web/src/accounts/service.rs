use accounts_contract::{
    NewPasswordRequest, PasswordResetRequest, SendCodeRequest, ServiceModeDTO, SignInRequest,
    SignUpRequest, SignedInDTO, VerifyEmailRequest,
};
use gloo_net::http::Request;

use crate::accounts::{SignInState, SignedInAccount};
use crate::http::{ApiError, checked, parsed, sent, sent_with};

const MODE: &str = "/api/service/mode";
const ACCOUNTS: &str = "/api/accounts";
const VERIFICATION: &str = "/api/accounts/verification";
const NEW_CODE: &str = "/api/accounts/verification/code";
const PASSWORD_RESET: &str = "/api/accounts/password-reset";
const NEW_PASSWORD: &str = "/api/accounts/password";
const SESSIONS: &str = "/api/sessions";
const CURRENT: &str = "/api/sessions/current";
const SUBJECT: &str = "session";

pub async fn current() -> Result<SignInState, ApiError> {
    let mode: ServiceModeDTO = parsed(sent(Request::get(MODE)).await?, SUBJECT).await?;
    if !mode.accounts {
        return Ok(SignInState::NoAccounts);
    }

    let response = sent(Request::get(CURRENT)).await?;
    match response.status() {
        401 => Ok(SignInState::SignedOut),
        _ => Ok(SignInState::SignedIn(as_signed_in_account(
            parsed(response, SUBJECT).await?,
        ))),
    }
}

pub async fn sign_in(email: &str, password: &str) -> Result<SignedInAccount, ApiError> {
    let payload = SignInRequest {
        email: email.to_owned(),
        password: password.to_owned(),
    };
    let response = sent_with(Request::post(SESSIONS), &payload).await?;

    Ok(as_signed_in_account(parsed(response, SUBJECT).await?))
}

pub async fn sign_up(email: &str, name: &str, password: &str) -> Result<(), ApiError> {
    let payload = SignUpRequest {
        email: email.to_owned(),
        name: name.to_owned(),
        password: password.to_owned(),
    };

    checked(sent_with(Request::post(ACCOUNTS), &payload).await?, SUBJECT)
        .await
        .map(|_| ())
}

pub async fn verify(email: &str, code: &str) -> Result<(), ApiError> {
    let payload = VerifyEmailRequest {
        email: email.to_owned(),
        code: code.to_owned(),
    };

    checked(
        sent_with(Request::post(VERIFICATION), &payload).await?,
        SUBJECT,
    )
    .await
    .map(|_| ())
}

pub async fn send_code_again(email: &str) -> Result<(), ApiError> {
    let payload = SendCodeRequest {
        email: email.to_owned(),
    };

    checked(sent_with(Request::post(NEW_CODE), &payload).await?, SUBJECT)
        .await
        .map(|_| ())
}

pub async fn ask_to_reset_password(email: &str) -> Result<(), ApiError> {
    let payload = PasswordResetRequest {
        email: email.to_owned(),
    };

    checked(
        sent_with(Request::post(PASSWORD_RESET), &payload).await?,
        SUBJECT,
    )
    .await
    .map(|_| ())
}

pub async fn reset_password(email: &str, code: &str, password: &str) -> Result<(), ApiError> {
    let payload = NewPasswordRequest {
        email: email.to_owned(),
        code: code.to_owned(),
        password: password.to_owned(),
    };

    checked(
        sent_with(Request::post(NEW_PASSWORD), &payload).await?,
        SUBJECT,
    )
    .await
    .map(|_| ())
}

pub async fn sign_out() -> Result<(), ApiError> {
    checked(sent(Request::delete(CURRENT)).await?, SUBJECT)
        .await
        .map(|_| ())
}

fn as_signed_in_account(signed_in: SignedInDTO) -> SignedInAccount {
    SignedInAccount {
        name: signed_in.name,
        email: signed_in.email,
    }
}
