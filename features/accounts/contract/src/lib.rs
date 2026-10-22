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

    pub const EVERY: [&str; 10] = [
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
    ];
}
