use accounts_contract::{
    AccountPageDTO, BecomeOperatorRequest, DeclineInvitationRequest, LockRequest,
    NewPasswordRequest, PasswordResetRequest, SendCodeRequest, ServiceModeDTO, SignInRequest,
    SignUpByInvitationRequest, SignUpRequest, SignedInDTO, TakeUpInvitationRequest,
    VerifyEmailRequest, refused,
};
use gloo_net::http::{Request, RequestBuilder};

use crate::accounts::listed::AccountPage;
use crate::accounts::{SignInState, SignedInAccount};
use crate::asking;
use crate::http;
use crate::http::ApiError;

const MODE: &str = "/api/service/mode";
const ACCOUNTS: &str = "/api/accounts";
const BY_INVITATION: &str = "/api/accounts/by-invitation";
const VERIFICATION: &str = "/api/accounts/verification";
const NEW_CODE: &str = "/api/accounts/verification/code";
const PASSWORD_RESET: &str = "/api/accounts/password-reset";
const NEW_PASSWORD: &str = "/api/accounts/password";
const SESSIONS: &str = "/api/sessions";
const CURRENT: &str = "/api/sessions/current";
const TAKEN_UP: &str = "/api/accounts/current/invitations/accept";
const MY_INVITATIONS: &str = "/api/accounts/current/invitations";
const TURNED_DOWN: &str = "/api/accounts/current/invitations/decline";
const OPERATORS: &str = "/api/operators";
const INSTANCE_ACCOUNTS: &str = "/api/instance/accounts";
const SUBJECT: &str = "session";
const ACCOUNT: &str = "account";
const IF_MATCH: &str = "If-Match";

fn made_on(request: RequestBuilder, on: u64) -> RequestBuilder {
    match on {
        0 => request,
        on => request.header(IF_MATCH, &format!("\"{on}\"")),
    }
}

pub async fn current() -> Result<(SignInState, ServiceModeDTO), ApiError> {
    let mode: ServiceModeDTO = http::parsed(http::sent(Request::get(MODE)).await?, SUBJECT).await?;
    if !mode.accounts {
        return Ok((SignInState::NoAccounts, mode));
    }

    let response = http::sent(Request::get(CURRENT)).await?;
    let state = match response.status() {
        401 => SignInState::SignedOut,
        _ => SignInState::SignedIn(as_signed_in_account(http::parsed(response, SUBJECT).await?)),
    };

    Ok((state, mode))
}

pub async fn take_up_invitation(token: &str) -> Result<(), ApiError> {
    let payload = TakeUpInvitationRequest {
        token: token.to_owned(),
    };

    http::checked(
        http::sent_with(Request::post(TAKEN_UP), &payload).await?,
        ACCOUNT,
    )
    .await
    .map(|_| ())
}

pub async fn decline_invitation(token: &str) -> Result<(), ApiError> {
    let payload = DeclineInvitationRequest {
        token: token.to_owned(),
    };

    http::checked(
        http::sent_with(Request::post(TURNED_DOWN), &payload).await?,
        ACCOUNT,
    )
    .await
    .map(|_| ())
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Answer {
    Accept,
    Decline,
}

fn answered_at(invitation: &str, answer: Answer) -> String {
    let answered = match answer {
        Answer::Accept => "accept",
        Answer::Decline => "decline",
    };

    format!("{MY_INVITATIONS}/{invitation}/{answered}")
}

/// Answers an invitation to the account, asking again a few times while it is refused as invalid: the list of one's invitations can show an invitation a moment before it can be taken up.
pub async fn answer_invitation(invitation: &str, answer: Answer) -> Result<(), ApiError> {
    asking::until_it_holds(
        || answered(invitation, answer),
        |answered| !not_yet_takeable(answered),
        asking::paused,
    )
    .await
}

async fn answered(invitation: &str, answer: Answer) -> Result<(), ApiError> {
    http::checked(
        http::sent(Request::post(&answered_at(invitation, answer))).await?,
        ACCOUNT,
    )
    .await
    .map(|_| ())
}

fn not_yet_takeable(answered: &Result<(), ApiError>) -> bool {
    matches!(answered, Err(ApiError::Rejected(refusal)) if refusal.kind.as_deref() == Some(refused::INVITATION_INVALID))
}

pub async fn sign_up_by_invitation(
    token: &str,
    name: &str,
    password: &str,
) -> Result<SignedInAccount, ApiError> {
    let payload = SignUpByInvitationRequest {
        token: token.to_owned(),
        name: name.to_owned(),
        password: password.to_owned(),
    };
    let response = http::sent_with(Request::post(BY_INVITATION), &payload).await?;

    Ok(as_signed_in_account(http::parsed(response, SUBJECT).await?))
}

pub async fn sign_in(email: &str, password: &str) -> Result<SignedInAccount, ApiError> {
    let payload = SignInRequest {
        email: email.to_owned(),
        password: password.to_owned(),
    };
    let response = http::sent_with(Request::post(SESSIONS), &payload).await?;

    Ok(as_signed_in_account(http::parsed(response, SUBJECT).await?))
}

pub async fn sign_up(
    email: &str,
    name: &str,
    password: &str,
    operator_code: Option<&str>,
) -> Result<(), ApiError> {
    let payload = SignUpRequest {
        email: email.to_owned(),
        name: name.to_owned(),
        password: password.to_owned(),
        operator_code: operator_code.map(str::to_owned),
    };

    http::checked(
        http::sent_with(Request::post(ACCOUNTS), &payload).await?,
        SUBJECT,
    )
    .await
    .map(|_| ())
}

pub async fn verify(email: &str, code: &str) -> Result<(), ApiError> {
    let payload = VerifyEmailRequest {
        email: email.to_owned(),
        code: code.to_owned(),
    };

    http::checked(
        http::sent_with(Request::post(VERIFICATION), &payload).await?,
        SUBJECT,
    )
    .await
    .map(|_| ())
}

pub async fn send_code_again(email: &str) -> Result<(), ApiError> {
    let payload = SendCodeRequest {
        email: email.to_owned(),
    };

    http::checked(
        http::sent_with(Request::post(NEW_CODE), &payload).await?,
        SUBJECT,
    )
    .await
    .map(|_| ())
}

pub async fn ask_to_reset_password(email: &str) -> Result<(), ApiError> {
    let payload = PasswordResetRequest {
        email: email.to_owned(),
    };

    http::checked(
        http::sent_with(Request::post(PASSWORD_RESET), &payload).await?,
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

    http::checked(
        http::sent_with(Request::post(NEW_PASSWORD), &payload).await?,
        SUBJECT,
    )
    .await
    .map(|_| ())
}

pub async fn become_operator(code: &str) -> Result<(), ApiError> {
    let payload = BecomeOperatorRequest {
        code: code.to_owned(),
    };

    http::checked(
        http::sent_with(Request::post(OPERATORS), &payload).await?,
        SUBJECT,
    )
    .await
    .map(|_| ())
}

pub async fn list_accounts(after: Option<&str>) -> Result<AccountPage, ApiError> {
    let asked = match after {
        Some(after) => format!("{INSTANCE_ACCOUNTS}?after={after}"),
        None => INSTANCE_ACCOUNTS.to_owned(),
    };
    let page: AccountPageDTO =
        http::parsed(http::sent(Request::get(&asked)).await?, ACCOUNT).await?;

    AccountPage::of(page)
}

pub async fn lock_account(id: &str, reason: &str, on: u64) -> Result<(), ApiError> {
    let payload = LockRequest {
        reason: reason.to_owned(),
    };

    http::checked(
        http::sent_with(made_on(Request::put(&locked(id)), on), &payload).await?,
        ACCOUNT,
    )
    .await
    .map(|_| ())
}

pub async fn unlock_account(id: &str, on: u64) -> Result<(), ApiError> {
    http::checked(
        http::sent(made_on(Request::delete(&locked(id)), on)).await?,
        ACCOUNT,
    )
    .await
    .map(|_| ())
}

pub async fn appoint_operator(id: &str, on: u64) -> Result<(), ApiError> {
    http::checked(
        http::sent(made_on(Request::put(&role(id)), on)).await?,
        ACCOUNT,
    )
    .await
    .map(|_| ())
}

pub async fn dismiss_operator(id: &str, on: u64) -> Result<(), ApiError> {
    http::checked(
        http::sent(made_on(Request::delete(&role(id)), on)).await?,
        ACCOUNT,
    )
    .await
    .map(|_| ())
}

fn role(id: &str) -> String {
    format!("{INSTANCE_ACCOUNTS}/{id}/operator")
}

fn locked(id: &str) -> String {
    format!("{INSTANCE_ACCOUNTS}/{id}/lock")
}

pub async fn sign_out() -> Result<(), ApiError> {
    http::checked(http::sent(Request::delete(CURRENT)).await?, SUBJECT)
        .await
        .map(|_| ())
}

fn as_signed_in_account(signed_in: SignedInDTO) -> SignedInAccount {
    SignedInAccount {
        account: signed_in.account,
        name: signed_in.name,
        email: signed_in.email,
        operator: signed_in.operator,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn an_invitation_is_accepted_or_declined_at_its_own_address() {
        // expect: each answer goes to the invitation's address under its own name
        assert_eq!(
            answered_at("invitation_1", Answer::Accept),
            "/api/accounts/current/invitations/invitation_1/accept"
        );
        assert_eq!(
            answered_at("invitation_1", Answer::Decline),
            "/api/accounts/current/invitations/invitation_1/decline"
        );
    }
}
