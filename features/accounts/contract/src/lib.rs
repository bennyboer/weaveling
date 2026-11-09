use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SignUpRequest {
    pub email: String,
    pub name: String,
    pub password: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operator_code: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SignUpByInvitationRequest {
    pub token: String,
    pub name: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TakeUpInvitationRequest {
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct DeclineInvitationRequest {
    pub token: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct VerifyEmailRequest {
    pub email: String,
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SendCodeRequest {
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PasswordResetRequest {
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct NewPasswordRequest {
    pub email: String,
    pub code: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SignInRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct BecomeOperatorRequest {
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct LockRequest {
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct LockDTO {
    pub kind: String,
    pub reason: String,
    pub since: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AccountSummaryDTO {
    pub id: String,
    #[serde(default)]
    pub version: u64,
    pub name: String,
    pub email: String,
    pub verified: bool,
    pub operator: bool,
    pub signed_up_at: String,
    pub locks: Vec<LockDTO>,
    #[serde(default)]
    pub second_factor: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AccountPageDTO {
    pub accounts: Vec<AccountSummaryDTO>,
    pub next: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SignedInDTO {
    pub account: String,
    pub name: String,
    pub email: String,
    pub stage: String,
    pub operator: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub second_factor: Option<SecondFactorDTO>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SecondFactorDTO {
    pub since: String,
    pub recovery_codes_left: usize,
    pub answered_by_totp: bool,
    pub answered_by_key: bool,
    pub preferred_way: String,
    pub asked_way: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PreferredWayRequest {
    pub preferred_way: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SettingUpTotpDTO {
    pub secret: String,
    pub otpauth_uri: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct TotpCodeRequest {
    pub code: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RecoveryCodesDTO {
    pub codes: Vec<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ServiceModeDTO {
    #[serde(default)]
    pub accounts: Option<AccountsModeDTO>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AccountsModeDTO {
    pub sign_up: String,
    pub operator_wanted: bool,
    pub can_set_up_totp: bool,
    pub can_add_keys: bool,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct KeyDTO {
    pub key: String,
    pub name: String,
    pub role: String,
    pub since: String,
}

/// An account's ways in as the operator sees them; the password, always kept, goes without saying.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct WaysInDTO {
    pub version: u64,
    pub keys: Vec<KeyDTO>,
    pub totp_since: Option<String>,
    pub recovery_codes_left: Option<usize>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct KeyChallengeDTO {
    pub options: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PasskeyChallengeDTO {
    pub signing_in: String,
    pub options: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PasskeySignInRequest {
    pub signing_in: String,
    pub answer: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct KeyChallengeRequest {
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub replacing: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PromoteKeyRequest {
    pub answer: serde_json::Value,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct KeyAnswerRequest {
    pub answer: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AddKeyRequest {
    pub name: String,
    pub answer: serde_json::Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct AddedKeyDTO {
    pub key: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub recovery_codes: Option<Vec<String>>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct RenameKeyRequest {
    pub name: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PasswordRequest {
    pub password: String,
}

pub const SESSION_COOKIE: &str = "weaveling_session";

pub const SIGNED_UP: &str = "account.signed.up";
pub const SIGNED_UP_BY_INVITATION: &str = "account.signed.up.by.invitation";
pub const EMAIL_VERIFIED: &str = "account.email.verified";
pub const PASSWORD_RESET: &str = "account.password.reset";
pub const BECAME_OPERATOR: &str = "account.became.operator";
pub const STOPPED_BEING_OPERATOR: &str = "account.stopped.being.operator";
pub const LOCKED: &str = "account.locked";
pub const UNLOCKED: &str = "account.unlocked";
pub const TOTP_ADDED: &str = "account.totp.added";
pub const TOTP_REMOVED: &str = "account.totp.removed";
pub const KEY_ADDED: &str = "account.key.added";
pub const KEY_RENAMED: &str = "account.key.renamed";
pub const KEY_REMOVED: &str = "account.key.removed";
pub const KEY_DEMOTED: &str = "account.key.demoted";
pub const INVITATION_DECLINED: &str = "account.invitation.declined";
pub const INVITATION_TAKEN_UP: &str = "account.invitation.taken.up";
pub const EVERY_ACCOUNT: &str = "account.#";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct PersonalDTO {
    pub name: String,
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "name")]
pub enum AccountEventDTO {
    #[serde(rename = "SIGNED_UP")]
    SignedUp { personal: PersonalDTO },
    #[serde(rename = "SIGNED_UP_BY_INVITATION")]
    SignedUpByInvitation { personal: PersonalDTO },
    #[serde(rename = "EMAIL_VERIFIED")]
    EmailVerified,
    #[serde(rename = "PASSWORD_RESET")]
    PasswordReset,
    #[serde(rename = "BECAME_OPERATOR")]
    BecameOperator,
    #[serde(rename = "STOPPED_BEING_OPERATOR")]
    StoppedBeingOperator,
    #[serde(rename = "LOCKED")]
    Locked { kind: String },
    #[serde(rename = "UNLOCKED")]
    Unlocked { kind: String },
    #[serde(rename = "INVITATION_DECLINED")]
    InvitationDeclined { invitation: String, reason: String },
    #[serde(rename = "INVITATION_TAKEN_UP")]
    InvitationTakenUp { invitation: String },
    #[serde(rename = "TOTP_ADDED")]
    TotpAdded,
    #[serde(rename = "TOTP_REMOVED")]
    TotpRemoved,
    #[serde(rename = "KEY_ADDED")]
    KeyAdded { key: String, role: String },
    #[serde(rename = "KEY_RENAMED")]
    KeyRenamed { key: String },
    #[serde(rename = "KEY_REMOVED")]
    KeyRemoved { key: String },
    #[serde(rename = "KEY_DEMOTED")]
    KeyDemoted { key: String },
}

pub mod sign_up {
    pub const OPEN: &str = "open";
    pub const BY_INVITATION: &str = "invitation";
}

pub mod second_step_way {
    pub const TOTP: &str = "totp";
    pub const KEY: &str = "key";
    pub const LAST_USED: &str = "last-used";
}

pub mod key_role {
    pub const PRIMARY: &str = "primary";
    pub const SECOND_FACTOR: &str = "second-factor";
}

pub mod decline_reason {
    pub const ADDRESS_HAS_ACCOUNT: &str = "address-has-account";
    pub const TURNED_DOWN: &str = "turned-down";
}

pub mod lock_kind {
    pub const OPERATOR: &str = "operator";
}

pub mod stage {
    pub const SIGNED_IN: &str = "signed-in";
    pub const SECOND_FACTOR_OWED: &str = "second-factor-owed";
}

pub mod refused {
    pub const EMAIL_MALFORMED: &str = "email-malformed";
    pub const EMAIL_TOO_LONG: &str = "email-too-long";
    pub const NAME_BLANK: &str = "account-name-blank";
    pub const NAME_CONTROL_CHARACTER: &str = "account-name-control-character";
    pub const NAME_TOO_LONG: &str = "account-name-too-long";
    pub const PASSWORD_TOO_SHORT: &str = "password-too-short";
    pub const PASSWORD_TOO_LONG: &str = "password-too-long";
    pub const CODE_WRONG: &str = "verification-code-wrong";
    pub const CODE_EXPIRED: &str = "verification-code-expired";
    pub const CODE_EXHAUSTED: &str = "verification-code-exhausted";
    pub const SIGN_IN_REFUSED: &str = "sign-in-refused";
    pub const NOT_VERIFIED: &str = "address-not-verified";
    pub const NOT_SIGNED_IN: &str = "not-signed-in";
    pub const THROTTLED: &str = "too-many-tries";
    pub const OPERATOR_CODE_WRONG: &str = "operator-code-wrong";
    pub const ACCOUNT_LOCKED: &str = "account-locked";
    pub const NOT_AN_OPERATOR: &str = "not-an-operator";
    pub const OPERATOR_CANNOT_BE_LOCKED: &str = "operator-cannot-be-locked";
    pub const LOCK_REASON_BLANK: &str = "lock-reason-blank";
    pub const LOCK_REASON_CONTROL_CHARACTER: &str = "lock-reason-control-character";
    pub const LOCK_REASON_TOO_LONG: &str = "lock-reason-too-long";
    pub const ACCOUNT_ID_INVALID: &str = "account-id-invalid";
    pub const ACCOUNT_NOT_FOUND: &str = "account-not-found";
    pub const LAST_OPERATOR: &str = "last-operator";
    pub const OPERATOR_ADDRESS_NOT_VERIFIED: &str = "operator-address-not-verified";
    pub const LOCKED_CANNOT_BECOME_OPERATOR: &str = "locked-cannot-become-operator";
    pub const BY_INVITATION_ONLY: &str = "signing-up-by-invitation-only";
    pub const INVITATION_INVALID: &str = "invitation-invalid";
    pub const INVITATION_EXPIRED: &str = "invitation-expired";
    pub const INVITATION_PAUSED: &str = "invitation-paused";
    pub const EMAIL_ADDRESS_HAS_ACCOUNT: &str = "email-address-has-account";
    pub const INVITATION_FOR_ANOTHER_ADDRESS: &str = "invitation-for-another-address";
    pub const INVITATION_NOT_BY_AN_OPERATOR: &str = "invitation-not-by-an-operator";
    pub const TOTP_UNAVAILABLE: &str = "totp-unavailable";
    pub const NOTHING_BEING_SET_UP: &str = "totp-nothing-being-set-up";
    pub const TOTP_CODE_WRONG: &str = "totp-code-wrong";
    pub const RECOVERY_CODE_WRONG: &str = "recovery-code-wrong";
    pub const NO_SECOND_FACTOR: &str = "no-second-factor";
    pub const KEYS_UNAVAILABLE: &str = "keys-unavailable";
    pub const NO_KEY_BEING_ADDED: &str = "no-key-being-added";
    pub const NO_KEY_BEING_ASKED: &str = "no-key-being-asked";
    pub const KEY_REFUSED: &str = "key-refused";
    pub const KEY_HELD_ALREADY: &str = "key-held-already";
    pub const NO_SUCH_KEY: &str = "no-such-key";
    pub const KEY_ID_INVALID: &str = "key-id-invalid";
    pub const KEY_NAME_TAKEN: &str = "key-name-taken";
    pub const KEY_NAME_BLANK: &str = "key-name-blank";
    pub const KEY_NAME_CONTROL_CHARACTER: &str = "key-name-control-character";
    pub const KEY_NAME_TOO_LONG: &str = "key-name-too-long";
    pub const TOO_MANY_KEYS: &str = "too-many-keys";
    pub const PASSWORD_WRONG: &str = "password-wrong";
    pub const PREFERRED_WAY_UNKNOWN: &str = "preferred-way-unknown";
    pub const KEY_ROLE_UNKNOWN: &str = "key-role-unknown";

    pub const EVERY: [&str; 53] = [
        EMAIL_MALFORMED,
        EMAIL_TOO_LONG,
        NAME_BLANK,
        NAME_CONTROL_CHARACTER,
        NAME_TOO_LONG,
        PASSWORD_TOO_SHORT,
        PASSWORD_TOO_LONG,
        CODE_WRONG,
        CODE_EXPIRED,
        CODE_EXHAUSTED,
        SIGN_IN_REFUSED,
        NOT_VERIFIED,
        NOT_SIGNED_IN,
        THROTTLED,
        OPERATOR_CODE_WRONG,
        ACCOUNT_LOCKED,
        NOT_AN_OPERATOR,
        OPERATOR_CANNOT_BE_LOCKED,
        LOCK_REASON_BLANK,
        LOCK_REASON_CONTROL_CHARACTER,
        LOCK_REASON_TOO_LONG,
        ACCOUNT_ID_INVALID,
        ACCOUNT_NOT_FOUND,
        LAST_OPERATOR,
        OPERATOR_ADDRESS_NOT_VERIFIED,
        LOCKED_CANNOT_BECOME_OPERATOR,
        BY_INVITATION_ONLY,
        INVITATION_INVALID,
        INVITATION_EXPIRED,
        INVITATION_PAUSED,
        EMAIL_ADDRESS_HAS_ACCOUNT,
        INVITATION_FOR_ANOTHER_ADDRESS,
        INVITATION_NOT_BY_AN_OPERATOR,
        TOTP_UNAVAILABLE,
        NOTHING_BEING_SET_UP,
        TOTP_CODE_WRONG,
        RECOVERY_CODE_WRONG,
        NO_SECOND_FACTOR,
        KEYS_UNAVAILABLE,
        NO_KEY_BEING_ADDED,
        NO_KEY_BEING_ASKED,
        KEY_REFUSED,
        KEY_HELD_ALREADY,
        NO_SUCH_KEY,
        KEY_ID_INVALID,
        KEY_NAME_TAKEN,
        KEY_NAME_BLANK,
        KEY_NAME_CONTROL_CHARACTER,
        KEY_NAME_TOO_LONG,
        TOO_MANY_KEYS,
        PASSWORD_WRONG,
        PREFERRED_WAY_UNKNOWN,
        KEY_ROLE_UNKNOWN,
    ];
}
