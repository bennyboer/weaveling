use accounts_contract::{
    AccountPageDTO, AddKeyRequest, AddedKeyDTO, BecomeOperatorRequest, DeclineInvitationRequest,
    KeyAnswerRequest, KeyChallengeDTO, KeyChallengeRequest, KeyDTO, LockRequest,
    NewPasswordRequest, PasskeyChallengeDTO, PasskeySignInRequest, PasswordRequest,
    PasswordResetRequest, PictureRequest, PreferredWayRequest, PromoteKeyRequest, RecoveryCodesDTO,
    RenameKeyRequest, SendCodeRequest, ServiceModeDTO, SettingUpTotpDTO, SignInRequest,
    SignUpByInvitationRequest, SignUpRequest, SignedInDTO, TakeUpInvitationRequest,
    TotpCodeRequest, VerifyEmailRequest, WaysInDTO, deletion_asked_by, key_role, refused,
    second_step_way, stage,
};
use gloo_net::http::Request;

use crate::accounts::listed::{AccountPage, ListedAccount, moment};
use crate::accounts::{
    HeldKey, PendingDeletion, PreferredWay, SecondFactorInUse, SecondStepWay, SignInState,
    SignedInAccount, WaysIn,
};
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
const DELETION: &str = "/api/accounts/current/deletion";
const TAKEN_UP: &str = "/api/accounts/current/invitations/accept";
const MY_INVITATIONS: &str = "/api/accounts/current/invitations";
const TURNED_DOWN: &str = "/api/accounts/current/invitations/decline";
const OPERATORS: &str = "/api/operators";
const INSTANCE_ACCOUNTS: &str = "/api/accounts";
const TOTP: &str = "/api/accounts/current/totp";
const KEYS: &str = "/api/accounts/current/keys";
const PREFERRED_WAY: &str = "/api/accounts/current/second-step/preferred";
const PICTURE: &str = "/api/accounts/current/picture";
const KEY_CHALLENGE: &str = "/api/accounts/current/keys/challenge";
const ANSWERED_BY_KEY: &str = "/api/sessions/current/key";
const KEY_ASKED: &str = "/api/sessions/current/key/challenge";
const PASSKEY_ASKED: &str = "/api/sessions/passkey/challenge";
const SIGNED_IN_BY_PASSKEY: &str = "/api/sessions/passkey";
const TOTP_SETUP: &str = "/api/accounts/current/totp/setup";
const RECOVERY_CODES: &str = "/api/accounts/current/recovery-codes";
const ANSWERED_BY_TOTP: &str = "/api/sessions/current/totp";
const ANSWERED_BY_RECOVERY_CODE: &str = "/api/sessions/current/recovery-code";
const TOTP_SETUP_FINISH: &str = "/api/accounts/current/totp/setup/finish";
const SUBJECT: &str = "session";
const ACCOUNT: &str = "account";

pub async fn current() -> Result<(SignInState, ServiceModeDTO), ApiError> {
    let mode: ServiceModeDTO = http::parsed(http::sent(Request::get(MODE)).await?, SUBJECT).await?;
    if mode.accounts.is_none() {
        return Ok((SignInState::NoAccounts, mode));
    }

    let response = http::sent(Request::get(CURRENT)).await?;
    let state = match response.status() {
        401 => SignInState::SignedOut,
        _ => match as_signed_in_account(http::parsed(response, SUBJECT).await?)? {
            account if account.owes_second_factor => SignInState::OwingSecondFactor(account),
            account if account.deletion.is_some() => SignInState::DeletionPending(account),
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

/// Answers an invitation sent to the account.
///
/// If the server refuses the invitation as invalid, the answer is sent again a few times.
/// This is because the list of invitations can show an invitation a moment before it can be accepted.
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

fn deleted(id: &str) -> String {
    format!("{INSTANCE_ACCOUNTS}/{id}/deletion")
}

pub async fn mark_for_deletion(id: &str, on: u64) -> Result<(), ApiError> {
    http::checked(
        http::sent(http::made_on(Request::put(&deleted(id)), on)).await?,
        ACCOUNT,
    )
    .await
    .map(|_| ())
}

pub async fn keep_account_of(id: &str, on: u64) -> Result<(), ApiError> {
    http::checked(
        http::sent(http::made_on(Request::delete(&deleted(id)), on)).await?,
        ACCOUNT,
    )
    .await
    .map(|_| ())
}

fn locked(id: &str) -> String {
    format!("{INSTANCE_ACCOUNTS}/{id}/lock")
}

pub async fn begin_setting_up_totp(password: &str) -> Result<SettingUpTotpDTO, ApiError> {
    let payload = PasswordRequest {
        password: password.to_owned(),
    };

    http::parsed(
        http::sent_with(Request::post(TOTP_SETUP), &payload).await?,
        SUBJECT,
    )
    .await
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

fn way_of(told: &str) -> Result<SecondStepWay, ApiError> {
    match told {
        second_step_way::TOTP => Ok(SecondStepWay::Totp),
        second_step_way::KEY => Ok(SecondStepWay::Key),
        _ => Err(ApiError::Unexpected),
    }
}

/// Shows another image as the signed-in account's picture.
/// With `drop_previous`, the picture shown before is deleted, if nothing else uses it.
pub async fn change_picture(picture: &str, drop_previous: bool) -> Result<(), ApiError> {
    let payload = PictureRequest {
        picture: picture.to_owned(),
        drop_previous,
    };

    http::checked(
        http::sent_with(Request::put(PICTURE), &payload).await?,
        SUBJECT,
    )
    .await
    .map(|_| ())
}

/// Shows no picture any more. With `drop`, the image is deleted too, if nothing else uses it.
pub async fn remove_picture(drop: bool) -> Result<(), ApiError> {
    let address = if drop {
        format!("{PICTURE}?drop=true")
    } else {
        PICTURE.to_owned()
    };

    http::checked(http::sent(Request::delete(&address)).await?, SUBJECT)
        .await
        .map(|_| ())
}

pub async fn choose_preferred_way(way: PreferredWay) -> Result<(), ApiError> {
    let payload = PreferredWayRequest {
        preferred_way: match way {
            PreferredWay::Chosen(SecondStepWay::Totp) => second_step_way::TOTP,
            PreferredWay::Chosen(SecondStepWay::Key) => second_step_way::KEY,
            PreferredWay::LastUsed => second_step_way::LAST_USED,
        }
        .to_owned(),
    };

    http::checked(
        http::sent_with(Request::put(PREFERRED_WAY), &payload).await?,
        SUBJECT,
    )
    .await
    .map(|_| ())
}

pub async fn begin_answering_with_key() -> Result<serde_json::Value, ApiError> {
    let asked: KeyChallengeDTO =
        http::parsed(http::sent(Request::post(KEY_ASKED)).await?, SUBJECT).await?;

    Ok(asked.options)
}

pub async fn answer_with_key(answer: serde_json::Value) -> Result<SignedInAccount, ApiError> {
    let response =
        http::sent_with(Request::post(ANSWERED_BY_KEY), &KeyAnswerRequest { answer }).await?;

    as_signed_in_account(http::parsed(response, SUBJECT).await?)
}

/// Starts signing in with a passkey.
/// Returns the id of this sign-in and the options the browser needs to show the passkeys of this site.
pub async fn begin_signing_in_with_passkey() -> Result<(String, serde_json::Value), ApiError> {
    let asked: PasskeyChallengeDTO =
        http::parsed(http::sent(Request::post(PASSKEY_ASKED)).await?, SUBJECT).await?;

    Ok((asked.signing_in, asked.options))
}

pub async fn sign_in_with_passkey(
    signing_in: String,
    answer: serde_json::Value,
) -> Result<SignedInAccount, ApiError> {
    let payload = PasskeySignInRequest { signing_in, answer };
    let response = http::sent_with(Request::post(SIGNED_IN_BY_PASSKEY), &payload).await?;

    as_signed_in_account(http::parsed(response, SUBJECT).await?)
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

pub async fn turn_off_totp(password: &str) -> Result<(), ApiError> {
    let payload = PasswordRequest {
        password: password.to_owned(),
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

/// Returns the ways an account can sign in, as the operator sees them.
pub async fn ways_in(id: &str) -> Result<WaysIn, ApiError> {
    let told: WaysInDTO = http::parsed(
        http::sent(Request::get(&format!("{INSTANCE_ACCOUNTS}/{id}/ways-in"))).await?,
        ACCOUNT,
    )
    .await?;

    Ok(WaysIn {
        keys: told
            .keys
            .into_iter()
            .map(held_key)
            .collect::<Result<_, _>>()?,
        totp_since: told.totp_since.as_deref().map(moment).transpose()?,
        recovery_codes_left: told.recovery_codes_left,
    })
}

pub async fn remove_key_of(id: &str, key: &str, on: u64) -> Result<(), ApiError> {
    http::checked(
        http::sent(http::made_on(
            Request::delete(&format!("{INSTANCE_ACCOUNTS}/{id}/keys/{key}")),
            on,
        ))
        .await?,
        ACCOUNT,
    )
    .await
    .map(|_| ())
}

pub async fn draw_recovery_codes(password: &str) -> Result<Vec<String>, ApiError> {
    let payload = PasswordRequest {
        password: password.to_owned(),
    };
    let drawn: RecoveryCodesDTO = http::parsed(
        http::sent_with(Request::post(RECOVERY_CODES), &payload).await?,
        SUBJECT,
    )
    .await?;

    Ok(drawn.codes)
}

/// Asks for the signed-in account to be deleted.
/// The account's password is needed for this.
/// Every session of the account ends with the deletion.
pub async fn ask_deletion(password: &str) -> Result<(), ApiError> {
    let payload = PasswordRequest {
        password: password.to_owned(),
    };

    http::checked(
        http::sent_with(Request::post(DELETION), &payload).await?,
        ACCOUNT,
    )
    .await
    .map(|_| ())
}

/// Cancels the pending deletion of the account and keeps it.
/// This is called from a session that signed in while the deletion was pending.
/// Returns the account, signed in again.
pub async fn keep_account() -> Result<SignedInAccount, ApiError> {
    as_signed_in_account(http::parsed(http::sent(Request::delete(DELETION)).await?, ACCOUNT).await?)
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
                answered_by_totp: factor.answered_by_totp,
                answered_by_key: factor.answered_by_key,
                preferred_way: match factor.preferred_way.as_str() {
                    second_step_way::LAST_USED => PreferredWay::LastUsed,
                    chosen => PreferredWay::Chosen(way_of(chosen)?),
                },
                asked_way: way_of(&factor.asked_way)?,
            })
        })
        .transpose()?;

    Ok(SignedInAccount {
        account: signed_in.account,
        name: signed_in.name,
        email: signed_in.email,
        operator: signed_in.operator,
        operator_wanted: signed_in.operator_wanted,
        second_factor,
        owes_second_factor: signed_in.stage == stage::SECOND_FACTOR_OWED,
        picture: signed_in.picture,
        deletion: match (signed_in.stage.as_str(), signed_in.deletion) {
            (stage::DELETION_PENDING, Some(deletion)) => Some(PendingDeletion {
                until: moment(&deletion.until)?,
                by_the_operator: deletion.by == deletion_asked_by::OPERATOR,
            }),
            _ => None,
        },
    })
}

/// Returns the account's keys that answer its second step, the oldest first.
pub async fn keys() -> Result<Vec<HeldKey>, ApiError> {
    let listed: Vec<KeyDTO> = http::parsed(http::sent(Request::get(KEYS)).await?, SUBJECT).await?;

    listed.into_iter().map(held_key).collect()
}

fn held_key(key: KeyDTO) -> Result<HeldKey, ApiError> {
    Ok(HeldKey {
        since: moment(&key.since)?,
        signs_in_alone: key.role == key_role::PRIMARY,
        id: key.key,
        name: key.name,
    })
}

/// Starts adding a passkey or a security key.
/// Returns the options the browser needs to make the key.
/// If `replacing` names a key, the new key takes its place.
pub async fn begin_adding_key(
    passkey: bool,
    replacing: Option<&str>,
    password: &str,
) -> Result<serde_json::Value, ApiError> {
    let payload = KeyChallengeRequest {
        role: if passkey {
            key_role::PRIMARY
        } else {
            key_role::SECOND_FACTOR
        }
        .to_owned(),
        replacing: replacing.map(ToOwned::to_owned),
        password: password.to_owned(),
    };
    let asked: KeyChallengeDTO = http::parsed(
        http::sent_with(Request::post(KEY_CHALLENGE), &payload).await?,
        SUBJECT,
    )
    .await?;

    Ok(asked.options)
}

pub async fn promote_key(
    id: &str,
    answer: serde_json::Value,
    password: &str,
) -> Result<(), ApiError> {
    let payload = PromoteKeyRequest {
        answer,
        password: password.to_owned(),
    };

    http::checked(
        http::sent_with(Request::post(&format!("{KEYS}/{id}/promote")), &payload).await?,
        SUBJECT,
    )
    .await
    .map(|_| ())
}

/// Turns a passkey into a key that answers the second step.
/// Returns the new recovery codes if this is the account's first second step.
pub async fn demote_key(id: &str, password: &str) -> Result<Option<Vec<String>>, ApiError> {
    let payload = PasswordRequest {
        password: password.to_owned(),
    };
    let demoted: AddedKeyDTO = http::parsed(
        http::sent_with(Request::post(&format!("{KEYS}/{id}/demote")), &payload).await?,
        SUBJECT,
    )
    .await?;

    Ok(demoted.recovery_codes)
}

/// Adds the key the browser made, under the given name.
/// Returns the new recovery codes if this is the account's first second step.
pub async fn finish_adding_key(
    name: &str,
    answer: serde_json::Value,
) -> Result<Option<Vec<String>>, ApiError> {
    let payload = AddKeyRequest {
        name: name.to_owned(),
        answer,
    };
    let added: AddedKeyDTO = http::parsed(
        http::sent_with(Request::post(KEYS), &payload).await?,
        SUBJECT,
    )
    .await?;

    Ok(added.recovery_codes)
}

pub async fn rename_key(id: &str, name: &str) -> Result<(), ApiError> {
    let payload = RenameKeyRequest {
        name: name.to_owned(),
    };

    http::checked(
        http::sent_with(Request::patch(&format!("{KEYS}/{id}")), &payload).await?,
        SUBJECT,
    )
    .await
    .map(|_| ())
}

pub async fn remove_key(id: &str, password: &str) -> Result<(), ApiError> {
    let payload = PasswordRequest {
        password: password.to_owned(),
    };

    http::checked(
        http::sent_with(Request::delete(&format!("{KEYS}/{id}")), &payload).await?,
        SUBJECT,
    )
    .await
    .map(|_| ())
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
