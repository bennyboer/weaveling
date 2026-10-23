use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SignUpRequest {
    pub email: String,
    pub name: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SignUpByInvitationRequest {
    pub token: String,
    pub name: String,
    pub password: String,
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
    pub name: String,
    pub email: String,
    pub verified: bool,
    pub operator: bool,
    pub signed_up_at: String,
    pub locks: Vec<LockDTO>,
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
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ServiceModeDTO {
    pub accounts: bool,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub sign_up: Option<String>,
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
pub const INVITATION_DECLINED: &str = "account.invitation.declined";
pub const EVERY_ACCOUNT: &str = "account.#";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "name")]
pub enum AccountEventDTO {
    #[serde(rename = "SIGNED_UP")]
    SignedUp,
    #[serde(rename = "SIGNED_UP_BY_INVITATION")]
    SignedUpByInvitation { invitation: String },
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
}

pub mod sign_up {
    pub const OPEN: &str = "open";
    pub const BY_INVITATION: &str = "invitation";
}

pub mod decline_reason {
    pub const ADDRESS_HAS_ACCOUNT: &str = "address-has-account";
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
    pub const EMAIL_ADDRESS_HAS_ACCOUNT: &str = "email-address-has-account";

    pub const EVERY: [&str; 30] = [
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
        EMAIL_ADDRESS_HAS_ACCOUNT,
    ];
}
