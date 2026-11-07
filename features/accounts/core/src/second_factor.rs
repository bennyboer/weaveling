use time::OffsetDateTime;

/// An account's second step, whatever answers it: since when it has one, and how many recovery codes are left.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecondFactor {
    pub since: OffsetDateTime,
    pub recovery_codes_left: usize,
}

/// The codes of an authenticator app answering an account's second step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Totp {
    pub since: OffsetDateTime,
}
