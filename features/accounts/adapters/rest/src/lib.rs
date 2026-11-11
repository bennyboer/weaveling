use accounts_contract::{
    AccountPageDTO, AccountSummaryDTO, AddKeyRequest, AddedKeyDTO, BecomeOperatorRequest,
    DeclineInvitationRequest, DeletionDTO, KeyAnswerRequest, KeyChallengeDTO, KeyChallengeRequest,
    KeyDTO, LockDTO, LockRequest, NewPasswordRequest, PasskeyChallengeDTO, PasskeySignInRequest,
    PasswordRequest, PasswordResetRequest, PreferredWayRequest, PromoteKeyRequest,
    RecoveryCodesDTO, RenameKeyRequest, SESSION_COOKIE, SecondFactorDTO, SendCodeRequest,
    SettingUpTotpDTO, SignInRequest, SignUpByInvitationRequest, SignUpRequest, SignedInDTO,
    TakeUpInvitationRequest, TotpCodeRequest, VerifyEmailRequest, WaysInDTO, deletion_asked_by,
    key_role, lock_kind, refused, second_step_way, stage,
};
use accounts_core::{
    AccountError, AccountId, AccountSummary, AccountsCore, AccountsError, AddedKey,
    DeletionAskedBy, InvalidAccountName, InvalidEmailAddress, InvalidKeyName, InvalidLockReason,
    InvalidPassword, Key, KeyId, KeyRole, LockKind, PendingDeletion, PreferredWay, SecondStepWay,
    Session, SessionToken, SignedIn, SigningIn, Stage,
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
use eventsourcing::{ServiceError, StoreError};
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
        .route("/sessions/current/totp", routing::post(answer_with_totp))
        .route("/sessions/current/key", routing::post(answer_with_key))
        .route(
            "/sessions/passkey/challenge",
            routing::post(begin_signing_in_with_passkey),
        )
        .route("/sessions/passkey", routing::post(sign_in_with_passkey))
        .route("/accounts/current/deletion", routing::delete(keep_account))
        .route(
            "/sessions/current/key/challenge",
            routing::post(begin_answering_with_key),
        )
        .route(
            "/sessions/current/recovery-code",
            routing::post(answer_with_recovery_code),
        )
        .with_state(accounts)
}

pub fn guarded_router(accounts: AccountsCore) -> Router {
    Router::new()
        .route("/operators", routing::post(become_operator))
        .route(
            "/accounts/current/invitations/accept",
            routing::post(take_up_invitation),
        )
        .route(
            "/accounts/current/invitations/{invitation}/accept",
            routing::post(take_up_invitation_by_id),
        )
        .route(
            "/accounts/current/invitations/decline",
            routing::post(decline_invitation),
        )
        .route(
            "/accounts/current/invitations/{invitation}/decline",
            routing::post(decline_invitation_by_id),
        )
        .route("/accounts/current/deletion", routing::post(ask_deletion))
        .route("/accounts/current/totp", routing::delete(turn_off_totp))
        .route(
            "/accounts/current/totp/setup",
            routing::post(begin_setting_up_totp),
        )
        .route(
            "/accounts/current/totp/setup/finish",
            routing::post(finish_setting_up_totp),
        )
        .route(
            "/accounts/current/recovery-codes",
            routing::post(draw_recovery_codes),
        )
        .route(
            "/accounts/current/keys",
            routing::get(list_keys).post(finish_adding_key),
        )
        .route(
            "/accounts/current/second-step/preferred",
            routing::put(choose_preferred_way),
        )
        .route(
            "/accounts/current/keys/challenge",
            routing::post(begin_adding_key),
        )
        .route(
            "/accounts/current/keys/{key}",
            routing::patch(rename_key).delete(remove_key),
        )
        .route(
            "/accounts/current/keys/{key}/promote",
            routing::post(promote_key),
        )
        .route(
            "/accounts/current/keys/{key}/demote",
            routing::post(demote_key),
        )
        .route("/instance/accounts", routing::get(list_accounts))
        .route(
            "/instance/accounts/{account}/totp",
            routing::delete(remove_totp),
        )
        .route(
            "/instance/accounts/{account}/ways-in",
            routing::get(ways_in),
        )
        .route(
            "/instance/accounts/{account}/keys/{key}",
            routing::delete(remove_key_of),
        )
        .route(
            "/instance/accounts/{account}/lock",
            routing::put(lock).delete(unlock),
        )
        .route(
            "/instance/accounts/{account}/operator",
            routing::put(appoint_operator).delete(dismiss_operator),
        )
        .route(
            "/instance/accounts/{account}/deletion",
            routing::put(mark_for_deletion).delete(keep_account_of),
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
            request
                .extensions_mut()
                .insert(accounts.session_end(&token, signed_in.session.until));
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
        Json(told(&accounts, &signed_in).await?),
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

    Ok((
        [(SET_COOKIE, holding(&token))],
        Json(told(&accounts, &signed_in).await?),
    )
        .into_response())
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

async fn take_up_invitation_by_id(
    State(accounts): State<AccountsCore>,
    Extension(account): Extension<AccountId>,
    Path(invitation): Path<String>,
) -> Result<StatusCode, Problem> {
    accounts
        .take_up_invitation_by_id(&account, &invitation)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn decline_invitation(
    State(accounts): State<AccountsCore>,
    Extension(account): Extension<AccountId>,
    Json(request): Json<DeclineInvitationRequest>,
) -> Result<StatusCode, Problem> {
    accounts
        .decline_invitation(&account, &request.token)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn decline_invitation_by_id(
    State(accounts): State<AccountsCore>,
    Extension(account): Extension<AccountId>,
    Path(invitation): Path<String>,
) -> Result<StatusCode, Problem> {
    accounts
        .decline_invitation_by_id(&account, &invitation)
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

async fn begin_setting_up_totp(
    State(accounts): State<AccountsCore>,
    Extension(account): Extension<AccountId>,
    Json(request): Json<PasswordRequest>,
) -> Result<Json<SettingUpTotpDTO>, Problem> {
    let setting_up = accounts
        .begin_setting_up_totp(&account, &request.password)
        .await
        .map_err(problem_of)?;

    Ok(Json(SettingUpTotpDTO {
        secret: setting_up.secret,
        otpauth_uri: setting_up.otpauth_uri,
    }))
}

async fn finish_setting_up_totp(
    State(accounts): State<AccountsCore>,
    Extension(account): Extension<AccountId>,
    headers: HeaderMap,
    Json(request): Json<TotpCodeRequest>,
) -> Result<Response, Problem> {
    let Some(session) = presented(&headers) else {
        return Err(not_signed_in());
    };
    let (token, codes) = accounts
        .finish_setting_up_totp(&account, &request.code, &session)
        .await
        .map_err(problem_of)?;

    Ok((
        [(SET_COOKIE, holding(&token))],
        Json(RecoveryCodesDTO {
            codes: codes.iter().map(ToString::to_string).collect(),
        }),
    )
        .into_response())
}

async fn turn_off_totp(
    State(accounts): State<AccountsCore>,
    Extension(account): Extension<AccountId>,
    headers: HeaderMap,
    Json(request): Json<PasswordRequest>,
) -> Result<Response, Problem> {
    let Some(session) = presented(&headers) else {
        return Err(not_signed_in());
    };
    let token = accounts
        .turn_off_totp(&account, &request.password, &session)
        .await
        .map_err(problem_of)?;

    Ok(([(SET_COOKIE, holding(&token))], StatusCode::NO_CONTENT).into_response())
}

async fn remove_totp(
    State(accounts): State<AccountsCore>,
    Extension(by): Extension<AccountId>,
    Path(account): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    accounts
        .remove_totp(&by, &account_named(&account)?, serving::demanded(&headers)?)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn list_keys(
    State(accounts): State<AccountsCore>,
    Extension(account): Extension<AccountId>,
) -> Result<Json<Vec<KeyDTO>>, Problem> {
    let keys = accounts.keys_of(&account).await.map_err(problem_of)?;

    Ok(Json(
        keys.iter().map(|(key, held)| key_told(key, held)).collect(),
    ))
}

fn key_told(key: &KeyId, held: &Key) -> KeyDTO {
    KeyDTO {
        key: key.to_string(),
        name: held.name.to_string(),
        role: match held.role {
            KeyRole::Primary => key_role::PRIMARY,
            KeyRole::SecondFactor => key_role::SECOND_FACTOR,
        }
        .to_owned(),
        since: text::serialize(held.since),
    }
}

async fn ways_in(
    State(accounts): State<AccountsCore>,
    Extension(by): Extension<AccountId>,
    Path(account): Path<String>,
) -> Result<Json<WaysInDTO>, Problem> {
    let ways = accounts
        .ways_in(&by, &account_named(&account)?)
        .await
        .map_err(problem_of)?;

    Ok(Json(WaysInDTO {
        version: ways.version.count(),
        keys: ways
            .keys
            .iter()
            .map(|(key, held)| key_told(key, held))
            .collect(),
        totp_since: ways.totp.map(|totp| text::serialize(totp.since)),
        recovery_codes_left: ways.recovery_codes_left,
    }))
}

async fn remove_key_of(
    State(accounts): State<AccountsCore>,
    Extension(by): Extension<AccountId>,
    Path((account, key)): Path<(String, String)>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    accounts
        .remove_key_of(
            &by,
            &account_named(&account)?,
            &key_named(&key)?,
            serving::demanded(&headers)?,
        )
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn begin_adding_key(
    State(accounts): State<AccountsCore>,
    Extension(account): Extension<AccountId>,
    Json(asked): Json<KeyChallengeRequest>,
) -> Result<Json<KeyChallengeDTO>, Problem> {
    let role = match asked.role.as_str() {
        key_role::SECOND_FACTOR => KeyRole::SecondFactor,
        key_role::PRIMARY => KeyRole::Primary,
        _ => {
            return Err(Problem::rejected(
                refused::KEY_ROLE_UNKNOWN,
                "a key signs in alone, as a passkey, or answers the second step",
            ));
        }
    };
    let replacing = asked.replacing.as_deref().map(key_named).transpose()?;
    let options = accounts
        .begin_adding_key(&account, role, replacing.as_ref(), &asked.password)
        .await
        .map_err(problem_of)?;

    Ok(Json(KeyChallengeDTO { options }))
}

async fn finish_adding_key(
    State(accounts): State<AccountsCore>,
    Extension(account): Extension<AccountId>,
    headers: HeaderMap,
    Json(request): Json<AddKeyRequest>,
) -> Result<Response, Problem> {
    let Some(session) = presented(&headers) else {
        return Err(not_signed_in());
    };
    let added = accounts
        .finish_adding_key(&account, &request.name, &request.answer, &session)
        .await
        .map_err(problem_of)?;

    Ok((
        StatusCode::CREATED,
        [(SET_COOKIE, holding(&added.session))],
        Json(AddedKeyDTO {
            key: added.key.to_string(),
            recovery_codes: added
                .recovery_codes
                .map(|codes| codes.iter().map(ToString::to_string).collect()),
        }),
    )
        .into_response())
}

async fn promote_key(
    State(accounts): State<AccountsCore>,
    Extension(account): Extension<AccountId>,
    Path(key): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PromoteKeyRequest>,
) -> Result<Response, Problem> {
    let Some(session) = presented(&headers) else {
        return Err(not_signed_in());
    };
    let promoted = accounts
        .replace_key_by_passkey(
            &account,
            &key_named(&key)?,
            &request.answer,
            &request.password,
            &session,
        )
        .await
        .map_err(problem_of)?;

    Ok(changed_key(&promoted))
}

async fn demote_key(
    State(accounts): State<AccountsCore>,
    Extension(account): Extension<AccountId>,
    Path(key): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PasswordRequest>,
) -> Result<Response, Problem> {
    let Some(session) = presented(&headers) else {
        return Err(not_signed_in());
    };
    let demoted = accounts
        .demote_key(&account, &key_named(&key)?, &request.password, &session)
        .await
        .map_err(problem_of)?;

    Ok(changed_key(&demoted))
}

fn changed_key(changed: &AddedKey) -> Response {
    (
        [(SET_COOKIE, holding(&changed.session))],
        Json(AddedKeyDTO {
            key: changed.key.to_string(),
            recovery_codes: changed
                .recovery_codes
                .as_ref()
                .map(|codes| codes.iter().map(ToString::to_string).collect()),
        }),
    )
        .into_response()
}

async fn rename_key(
    State(accounts): State<AccountsCore>,
    Extension(account): Extension<AccountId>,
    Path(key): Path<String>,
    Json(request): Json<RenameKeyRequest>,
) -> Result<StatusCode, Problem> {
    accounts
        .rename_key(&account, &key_named(&key)?, &request.name)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn remove_key(
    State(accounts): State<AccountsCore>,
    Extension(account): Extension<AccountId>,
    Path(key): Path<String>,
    headers: HeaderMap,
    Json(request): Json<PasswordRequest>,
) -> Result<Response, Problem> {
    let Some(session) = presented(&headers) else {
        return Err(not_signed_in());
    };
    let token = accounts
        .remove_key(&account, &key_named(&key)?, &request.password, &session)
        .await
        .map_err(problem_of)?;

    Ok(([(SET_COOKIE, holding(&token))], StatusCode::NO_CONTENT).into_response())
}

async fn draw_recovery_codes(
    State(accounts): State<AccountsCore>,
    Extension(account): Extension<AccountId>,
    Json(request): Json<PasswordRequest>,
) -> Result<Json<RecoveryCodesDTO>, Problem> {
    let codes = accounts
        .draw_recovery_codes(&account, &request.password)
        .await
        .map_err(problem_of)?;

    Ok(Json(RecoveryCodesDTO {
        codes: codes.iter().map(ToString::to_string).collect(),
    }))
}

async fn begin_signing_in_with_passkey(
    State(accounts): State<AccountsCore>,
) -> Result<Json<PasskeyChallengeDTO>, Problem> {
    let (signing_in, options) = accounts
        .begin_signing_in_with_passkey()
        .map_err(problem_of)?;

    Ok(Json(PasskeyChallengeDTO {
        signing_in: signing_in.as_str().to_owned(),
        options,
    }))
}

async fn sign_in_with_passkey(
    State(accounts): State<AccountsCore>,
    Json(request): Json<PasskeySignInRequest>,
) -> Result<Response, Problem> {
    let (token, signed_in) = accounts
        .sign_in_with_passkey(&SigningIn::told(&request.signing_in), &request.answer)
        .await
        .map_err(problem_of)?;

    Ok((
        [(SET_COOKIE, holding(&token))],
        Json(told(&accounts, &signed_in).await?),
    )
        .into_response())
}

async fn begin_answering_with_key(
    State(accounts): State<AccountsCore>,
    headers: HeaderMap,
) -> Result<Json<KeyChallengeDTO>, Problem> {
    let Some(owing) = presented(&headers) else {
        return Err(not_signed_in());
    };
    let options = accounts
        .begin_answering_with_key(&owing)
        .await
        .map_err(problem_of)?;

    Ok(Json(KeyChallengeDTO { options }))
}

async fn answer_with_key(
    State(accounts): State<AccountsCore>,
    headers: HeaderMap,
    Json(request): Json<KeyAnswerRequest>,
) -> Result<Response, Problem> {
    let Some(owing) = presented(&headers) else {
        return Err(not_signed_in());
    };
    let (token, signed_in) = accounts
        .answer_with_key(&owing, &request.answer)
        .await
        .map_err(problem_of)?;

    Ok((
        [(SET_COOKIE, holding(&token))],
        Json(told(&accounts, &signed_in).await?),
    )
        .into_response())
}

async fn answer_with_totp(
    State(accounts): State<AccountsCore>,
    headers: HeaderMap,
    Json(request): Json<TotpCodeRequest>,
) -> Result<Response, Problem> {
    let Some(owing) = presented(&headers) else {
        return Err(not_signed_in());
    };
    let (token, signed_in) = accounts
        .answer_with_totp(&owing, &request.code)
        .await
        .map_err(problem_of)?;

    Ok((
        [(SET_COOKIE, holding(&token))],
        Json(told(&accounts, &signed_in).await?),
    )
        .into_response())
}

async fn answer_with_recovery_code(
    State(accounts): State<AccountsCore>,
    headers: HeaderMap,
    Json(request): Json<TotpCodeRequest>,
) -> Result<Response, Problem> {
    let Some(owing) = presented(&headers) else {
        return Err(not_signed_in());
    };
    let (token, signed_in) = accounts
        .answer_with_recovery_code(&owing, &request.code)
        .await
        .map_err(problem_of)?;

    Ok((
        [(SET_COOKIE, holding(&token))],
        Json(told(&accounts, &signed_in).await?),
    )
        .into_response())
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
    headers: HeaderMap,
    Json(request): Json<LockRequest>,
) -> Result<StatusCode, Problem> {
    accounts
        .lock(
            &by,
            &account_named(&account)?,
            &request.reason,
            serving::demanded(&headers)?,
        )
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn unlock(
    State(accounts): State<AccountsCore>,
    Extension(by): Extension<AccountId>,
    Path(account): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    accounts
        .unlock(&by, &account_named(&account)?, serving::demanded(&headers)?)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn appoint_operator(
    State(accounts): State<AccountsCore>,
    Extension(by): Extension<AccountId>,
    Path(account): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    accounts
        .appoint_operator(&by, &account_named(&account)?, serving::demanded(&headers)?)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn dismiss_operator(
    State(accounts): State<AccountsCore>,
    Extension(by): Extension<AccountId>,
    Path(account): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    accounts
        .dismiss_operator(&by, &account_named(&account)?, serving::demanded(&headers)?)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

async fn mark_for_deletion(
    State(accounts): State<AccountsCore>,
    Extension(by): Extension<AccountId>,
    Path(account): Path<String>,
    headers: HeaderMap,
) -> Result<Json<DeletionDTO>, Problem> {
    let pending = accounts
        .mark_for_deletion(&by, &account_named(&account)?, serving::demanded(&headers)?)
        .await
        .map_err(problem_of)?;

    Ok(Json(deletion_told(pending)))
}

async fn keep_account_of(
    State(accounts): State<AccountsCore>,
    Extension(by): Extension<AccountId>,
    Path(account): Path<String>,
    headers: HeaderMap,
) -> Result<StatusCode, Problem> {
    accounts
        .keep_account_of(&by, &account_named(&account)?, serving::demanded(&headers)?)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
}

fn account_named(given: &str) -> Result<AccountId, Problem> {
    given
        .parse()
        .map_err(|_| Problem::rejected(refused::ACCOUNT_ID_INVALID, "this is no account's id"))
}

fn key_named(given: &str) -> Result<KeyId, Problem> {
    given
        .parse()
        .map_err(|_| Problem::rejected(refused::KEY_ID_INVALID, "this is no key's id"))
}

fn to_summary_dto(summary: &AccountSummary) -> AccountSummaryDTO {
    AccountSummaryDTO {
        deletion: summary.deletion.map(deletion_told),
        second_factor: summary.second_factor,
        id: summary.id.to_string(),
        version: summary.version.count(),
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

    let signed_in = accounts
        .signed_in(&token)
        .await
        .map_err(problem_of)?
        .ok_or_else(not_signed_in)?;

    Ok(Json(told(&accounts, &signed_in).await?))
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

async fn told(accounts: &AccountsCore, signed_in: &SignedIn) -> Result<SignedInDTO, Problem> {
    let asked_way = accounts
        .asked_way(&signed_in.session.account, &signed_in.account)
        .await
        .map_err(problem_of)?;

    Ok(SignedInDTO {
        account: signed_in.session.account.to_string(),
        name: signed_in.account.name().to_string(),
        email: signed_in.account.email().to_string(),
        stage: match signed_in.session.stage {
            Stage::SignedIn => stage::SIGNED_IN,
            Stage::SecondFactorOwed => stage::SECOND_FACTOR_OWED,
            Stage::DeletionPending => stage::DELETION_PENDING,
        }
        .to_owned(),
        operator: signed_in.account.is_operator(),
        deletion: signed_in.account.pending_deletion().map(deletion_told),
        second_factor: signed_in.account.second_factor().zip(asked_way).map(
            |(factor, asked_way)| SecondFactorDTO {
                since: text::serialize(factor.since),
                recovery_codes_left: factor.recovery_codes_left,
                answered_by_totp: factor.answered_by_totp,
                answered_by_key: factor.answered_by_key,
                preferred_way: match signed_in.account.preferred_way() {
                    PreferredWay::Chosen(way) => way_named(way),
                    PreferredWay::LastUsed => second_step_way::LAST_USED,
                }
                .to_owned(),
                asked_way: way_named(asked_way).to_owned(),
            },
        ),
    })
}

fn deletion_told(pending: PendingDeletion) -> DeletionDTO {
    DeletionDTO {
        until: text::serialize(pending.until),
        by: match pending.asked_by {
            DeletionAskedBy::Holder => deletion_asked_by::HOLDER,
            DeletionAskedBy::Operator => deletion_asked_by::OPERATOR,
        }
        .to_owned(),
    }
}

async fn ask_deletion(
    State(accounts): State<AccountsCore>,
    Extension(account): Extension<AccountId>,
    Json(request): Json<PasswordRequest>,
) -> Result<Response, Problem> {
    let pending = accounts
        .ask_deletion(&account, &request.password)
        .await
        .map_err(problem_of)?;

    Ok(([(SET_COOKIE, letting_go())], Json(deletion_told(pending))).into_response())
}

async fn keep_account(
    State(accounts): State<AccountsCore>,
    headers: HeaderMap,
) -> Result<Response, Problem> {
    let Some(pending) = presented(&headers) else {
        return Err(not_signed_in());
    };
    let (token, signed_in) = accounts.keep_account(&pending).await.map_err(problem_of)?;

    Ok((
        [(SET_COOKIE, holding(&token))],
        Json(told(&accounts, &signed_in).await?),
    )
        .into_response())
}

fn way_named(way: SecondStepWay) -> &'static str {
    match way {
        SecondStepWay::Totp => second_step_way::TOTP,
        SecondStepWay::Key => second_step_way::KEY,
    }
}

async fn choose_preferred_way(
    State(accounts): State<AccountsCore>,
    Extension(account): Extension<AccountId>,
    Json(request): Json<PreferredWayRequest>,
) -> Result<StatusCode, Problem> {
    let way = match request.preferred_way.as_str() {
        second_step_way::TOTP => PreferredWay::Chosen(SecondStepWay::Totp),
        second_step_way::KEY => PreferredWay::Chosen(SecondStepWay::Key),
        second_step_way::LAST_USED => PreferredWay::LastUsed,
        _ => {
            return Err(Problem::rejected(
                refused::PREFERRED_WAY_UNKNOWN,
                "a second step is asked first by totp, by key, or as it was answered last",
            ));
        }
    };
    accounts
        .choose_preferred_way(&account, way)
        .await
        .map_err(problem_of)?;

    Ok(StatusCode::NO_CONTENT)
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
        AccountsError::ProjectsToHandOver(ref projects) => {
            let projects = projects.clone();
            Problem::refused(refused::PROJECTS_TO_HAND_OVER, refusal).with("projects", projects)
        }
        AccountsError::NoDeletionPending => Problem::refused(refused::NO_DELETION_PENDING, refusal),
        AccountsError::Events(ServiceError::Refused(AccountError::DeletionAskedByTheOperator)) => {
            Problem::forbidden(refused::DELETION_ASKED_BY_THE_OPERATOR, refusal)
        }
        AccountsError::Events(ServiceError::Refused(AccountError::Gone)) => {
            Problem::not_found(refused::ACCOUNT_NOT_FOUND, "there is no such account")
        }
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
        AccountsError::Events(ServiceError::Store(StoreError::Outdated { ref kind, .. })) => {
            Problem::outdated(kind)
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
        AccountsError::InvitationPaused => Problem::refused(refused::INVITATION_PAUSED, refusal),
        AccountsError::InvitationForAnotherAddress => {
            Problem::forbidden(refused::INVITATION_FOR_ANOTHER_ADDRESS, refusal)
        }
        AccountsError::InvitationNotByAnOperator => {
            Problem::forbidden(refused::INVITATION_NOT_BY_AN_OPERATOR, refusal)
        }
        AccountsError::EmailAddressHasAccount => {
            Problem::refused(refused::EMAIL_ADDRESS_HAS_ACCOUNT, refusal)
        }
        AccountsError::TotpUnavailable => Problem::refused(refused::TOTP_UNAVAILABLE, refusal),
        AccountsError::NothingBeingSetUp => {
            Problem::refused(refused::NOTHING_BEING_SET_UP, refusal)
        }
        AccountsError::TotpCodeWrong => Problem::unprocessable(refused::TOTP_CODE_WRONG, refusal),
        AccountsError::RecoveryCodeWrong => {
            Problem::unprocessable(refused::RECOVERY_CODE_WRONG, refusal)
        }
        AccountsError::NoSecondFactor => Problem::refused(refused::NO_SECOND_FACTOR, refusal),
        AccountsError::NoSecondFactorOwed => not_signed_in(),
        AccountsError::KeysUnavailable => Problem::refused(refused::KEYS_UNAVAILABLE, refusal),
        AccountsError::NoKeyBeingAdded => Problem::refused(refused::NO_KEY_BEING_ADDED, refusal),
        AccountsError::NoKeyBeingAsked => Problem::refused(refused::NO_KEY_BEING_ASKED, refusal),
        AccountsError::KeyRefused => Problem::unprocessable(refused::KEY_REFUSED, refusal),
        AccountsError::KeyHeldAlready => Problem::refused(refused::KEY_HELD_ALREADY, refusal),
        AccountsError::NoSuchKey => Problem::not_found(refused::NO_SUCH_KEY, refusal),
        AccountsError::PasswordWrong => Problem::unprocessable(refused::PASSWORD_WRONG, refusal),
        AccountsError::Events(ServiceError::Refused(AccountError::KeyNameTaken)) => {
            Problem::refused(refused::KEY_NAME_TAKEN, refusal)
        }
        AccountsError::Events(ServiceError::Refused(AccountError::TooManyKeys)) => {
            Problem::refused(refused::TOO_MANY_KEYS, refusal)
        }
        AccountsError::Events(ServiceError::Refused(AccountError::NoSecondFactor)) => {
            Problem::refused(refused::NO_SECOND_FACTOR, refusal)
        }
        AccountsError::Events(ServiceError::Refused(AccountError::NoSuchKey)) => {
            Problem::not_found(refused::NO_SUCH_KEY, refusal)
        }
        AccountsError::InvalidKeyName(reason) => match reason {
            InvalidKeyName::Blank => Problem::unprocessable(refused::KEY_NAME_BLANK, reason),
            InvalidKeyName::ControlCharacter => {
                Problem::unprocessable(refused::KEY_NAME_CONTROL_CHARACTER, reason)
            }
            InvalidKeyName::TooLong { max, actual } => {
                Problem::unprocessable(refused::KEY_NAME_TOO_LONG, reason)
                    .with("max", max)
                    .with("actual", actual)
            }
        },
        unserveable => Problem::unexpected(unserveable),
    }
}
