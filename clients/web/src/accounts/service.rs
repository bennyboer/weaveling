use accounts_contract::{
    AccountPageDTO, BecomeOperatorRequest, DeclineInvitationRequest, LockRequest,
    NewPasswordRequest, PasswordResetRequest, RecoveryCodesDTO, SendCodeRequest, ServiceModeDTO,
    SettingUpTotpDTO, SignInRequest, SignUpByInvitationRequest, SignUpRequest, SignedInDTO,
    TakeUpInvitationRequest, TotpCodeRequest, VerifyEmailRequest, refused, stage,
};
use gloo_net::http::Request;

use crate::accounts::listed::{AccountPage, ListedAccount, moment};
use crate::accounts::{SecondFactorInUse, SignInState, SignedInAccount};
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
const TOTP: &str = "/api/accounts/current/totp";
const TOTP_SETUP: &str = "/api/accounts/current/totp/setup";
const RECOVERY_CODES: &str = "/api/accounts/current/recovery-codes";
const ANSWERED_BY_TOTP: &str = "/api/sessions/current/totp";
const ANSWERED_BY_RECOVERY_CODE: &str = "/api/sessions/current/recovery-code";
const TOTP_SETUP_FINISH: &str = "/api/accounts/current/totp/setup/finish";
const SUBJECT: &str = "session";
const ACCOUNT: &str = "account";

pub async fn current() -> Result<(SignInState, ServiceModeDTO), ApiError> {
    let mode: ServiceModeDTO = http::parsed(http::sent(Request::get(MODE)).await?, SUBJECT).await?;
    if !mode.accounts {
        return Ok((SignInState::NoAccounts, mode));
    }

    let response = http::sent(Request::get(CURRENT)).await?;
    let state = match response.status() {
        401 => SignInState::SignedOut,
        _ => match as_signed_in_account(http::parsed(response, SUBJECT).await?)? {
            account if account.owes_second_factor => SignInState::OwingSecondFactor(account),
            account => SignInState::SignedIn(account),
        },
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
        || invitation_answered(invitation, answer),
        |answered| !not_yet_takeable(answered),
        asking::paused,
    )
    .await
}

async fn invitation_answered(invitation: &str, answer: Answer) -> Result<(), ApiError> {
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

    as_signed_in_account(http::parsed(response, SUBJECT).await?)
}

pub async fn sign_in(email: &str, password: &str) -> Result<SignedInAccount, ApiError> {
    let payload = SignInRequest {
        email: email.to_owned(),
        password: password.to_owned(),
    };
    let response = http::sent_with(Request::post(SESSIONS), &payload).await?;

    as_signed_in_account(http::parsed(response, SUBJECT).await?)
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

pub async fn listed_account(id: &str) -> Result<ListedAccount, ApiError> {
    let mut after = None;
    loop {
        let page = list_accounts(after.as_deref()).await?;
        if let Some(found) = page.accounts.into_iter().find(|account| account.id == id) {
            return Ok(found);
        }
        after = Some(page.next.ok_or(ApiError::NotFound(ACCOUNT))?);
    }
}

pub async fn lock_account(id: &str, reason: &str, on: u64) -> Result<(), ApiError> {
    let payload = LockRequest {
        reason: reason.to_owned(),
    };

    http::checked(
        http::sent_with(http::made_on(Request::put(&locked(id)), on), &payload).await?,
        ACCOUNT,
    )
    .await
    .map(|_| ())
}

pub async fn unlock_account(id: &str, on: u64) -> Result<(), ApiError> {
    http::checked(
        http::sent(http::made_on(Request::delete(&locked(id)), on)).await?,
        ACCOUNT,
    )
    .await
    .map(|_| ())
}

pub async fn appoint_operator(id: &str, on: u64) -> Result<(), ApiError> {
    http::checked(
        http::sent(http::made_on(Request::put(&role(id)), on)).await?,
        ACCOUNT,
    )
    .await
    .map(|_| ())
}

pub async fn dismiss_operator(id: &str, on: u64) -> Result<(), ApiError> {
    http::checked(
        http::sent(http::made_on(Request::delete(&role(id)), on)).await?,
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

pub async fn begin_setting_up_totp() -> Result<SettingUpTotpDTO, ApiError> {
    http::parsed(http::sent(Request::post(TOTP_SETUP)).await?, SUBJECT).await
}

pub async fn finish_setting_up_totp(code: &str) -> Result<Vec<String>, ApiError> {
    let payload = TotpCodeRequest {
        code: code.to_owned(),
    };
    let shown: RecoveryCodesDTO = http::parsed(
        http::sent_with(Request::post(TOTP_SETUP_FINISH), &payload).await?,
        SUBJECT,
    )
    .await?;

    Ok(shown.codes)
}

pub async fn answer_with_totp(code: &str) -> Result<SignedInAccount, ApiError> {
    second_factor_answered(ANSWERED_BY_TOTP, code).await
}

pub async fn answer_with_recovery_code(code: &str) -> Result<SignedInAccount, ApiError> {
    second_factor_answered(ANSWERED_BY_RECOVERY_CODE, code).await
}

async fn second_factor_answered(at: &str, code: &str) -> Result<SignedInAccount, ApiError> {
    let payload = TotpCodeRequest {
        code: code.to_owned(),
    };
    let response = http::sent_with(Request::post(at), &payload).await?;

    as_signed_in_account(http::parsed(response, SUBJECT).await?)
}

pub async fn turn_off_totp(code: &str) -> Result<(), ApiError> {
    let payload = TotpCodeRequest {
        code: code.to_owned(),
    };

    http::checked(
        http::sent_with(Request::delete(TOTP), &payload).await?,
        SUBJECT,
    )
    .await
    .map(|_| ())
}

pub async fn remove_totp(id: &str, on: u64) -> Result<(), ApiError> {
    http::checked(
        http::sent(http::made_on(
            Request::delete(&format!("{INSTANCE_ACCOUNTS}/{id}/totp")),
            on,
        ))
        .await?,
        ACCOUNT,
    )
    .await
    .map(|_| ())
}

pub async fn draw_recovery_codes(code: &str) -> Result<Vec<String>, ApiError> {
    let payload = TotpCodeRequest {
        code: code.to_owned(),
    };
    let drawn: RecoveryCodesDTO = http::parsed(
        http::sent_with(Request::post(RECOVERY_CODES), &payload).await?,
        SUBJECT,
    )
    .await?;

    Ok(drawn.codes)
}

pub async fn sign_out() -> Result<(), ApiError> {
    http::checked(http::sent(Request::delete(CURRENT)).await?, SUBJECT)
        .await
        .map(|_| ())
}

fn as_signed_in_account(signed_in: SignedInDTO) -> Result<SignedInAccount, ApiError> {
    let second_factor = signed_in
        .second_factor
        .map(|factor| {
            Ok::<_, ApiError>(SecondFactorInUse {
                since: moment(&factor.since)?,
                recovery_codes_left: factor.recovery_codes_left,
            })
        })
        .transpose()?;

    Ok(SignedInAccount {
        account: signed_in.account,
        name: signed_in.name,
        email: signed_in.email,
        operator: signed_in.operator,
        second_factor,
        owes_second_factor: signed_in.stage == stage::SECOND_FACTOR_OWED,
    })
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
