use accounts_contract::{SignInRequest, SignedInDTO};
use gloo_net::http::Request;

use crate::accounts::{SignInState, SignedInAccount};
use crate::http::{ApiError, checked, parsed, sent, sent_with};

const SESSIONS: &str = "/api/sessions";
const CURRENT: &str = "/api/sessions/current";
const SUBJECT: &str = "session";

pub async fn current() -> Result<SignInState, ApiError> {
    let response = sent(Request::get(CURRENT)).await?;

    match response.status() {
        401 => Ok(SignInState::SignedOut),
        404 => Ok(SignInState::NoAccounts),
        _ => Ok(SignInState::SignedIn(as_who(
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

    Ok(as_who(parsed(response, SUBJECT).await?))
}

pub async fn sign_out() -> Result<(), ApiError> {
    checked(sent(Request::delete(CURRENT)).await?, SUBJECT)
        .await
        .map(|_| ())
}

fn as_who(signed_in: SignedInDTO) -> SignedInAccount {
    SignedInAccount {
        name: signed_in.name,
        email: signed_in.email,
    }
}
