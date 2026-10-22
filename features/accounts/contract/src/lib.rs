use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SignUpRequest {
    pub email: String,
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
pub struct SignInRequest {
    pub email: String,
    pub password: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct SignedInDTO {
    pub account: String,
    pub name: String,
    pub email: String,
    pub stage: String,
}

pub const SESSION_COOKIE: &str = "weaveling_session";

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

    pub const EVERY: [&str; 13] = [
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
    ];
}
