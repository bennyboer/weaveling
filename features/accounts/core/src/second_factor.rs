use time::OffsetDateTime;

/// An account's second step: since when it has one, how many recovery codes are left, and what else answers it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct SecondFactor {
    pub since: OffsetDateTime,
    pub recovery_codes_left: usize,
    pub answered_by_totp: bool,
    pub answered_by_key: bool,
}

/// A way of answering the second step, recovery codes aside, which are never asked first.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum SecondStepWay {
    Totp,
    Key,
}

/// Which way the second step is asked first, as the account chose: one way, or whichever answered it last.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash)]
pub enum PreferredWay {
    Chosen(SecondStepWay),
    #[default]
    LastUsed,
}

/// The codes of an authenticator app answering an account's second step.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Totp {
    pub since: OffsetDateTime,
}
