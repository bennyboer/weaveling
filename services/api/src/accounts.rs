use std::env;
use std::fmt::{self, Display, Formatter};

use thiserror::Error;

pub const WEAVELING_ACCOUNTS: &str = "WEAVELING_ACCOUNTS";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accounts {
    On,
    Off,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("{WEAVELING_ACCOUNTS} is '{given}'; set it to on or off")]
pub struct UnreadableAccounts {
    pub given: String,
}

impl Accounts {
    pub fn chosen(given: Option<String>) -> Result<Self, UnreadableAccounts> {
        let Some(given) = given else {
            return Ok(Self::Off);
        };

        match given.trim().to_lowercase().as_str() {
            "on" => Ok(Self::On),
            "off" | "" => Ok(Self::Off),
            _ => Err(UnreadableAccounts { given }),
        }
    }

    pub fn from_environment() -> Result<Self, UnreadableAccounts> {
        Self::chosen(env::var(WEAVELING_ACCOUNTS).ok())
    }
}

impl Display for Accounts {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::On => write!(f, "with accounts"),
            Self::Off => write!(f, "without accounts"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(value: &str) -> Option<String> {
        Some(value.to_owned())
    }

    #[test]
    fn nothing_set_means_no_accounts() {
        assert_eq!(
            Accounts::chosen(None),
            Ok(Accounts::Off),
            "local mode is what needs no setting, and must never show accounts by accident"
        );
    }

    #[test]
    fn on_and_off_are_read_however_they_are_written() {
        assert_eq!(Accounts::chosen(set("on")), Ok(Accounts::On));
        assert_eq!(Accounts::chosen(set(" ON ")), Ok(Accounts::On));
        assert_eq!(Accounts::chosen(set("off")), Ok(Accounts::Off));
        assert_eq!(Accounts::chosen(set("Off")), Ok(Accounts::Off));
    }

    #[test]
    fn a_blank_setting_counts_as_unset() {
        assert_eq!(Accounts::chosen(set("  ")), Ok(Accounts::Off));
    }

    #[test]
    fn anything_else_is_refused_rather_than_guessed() {
        assert_eq!(
            Accounts::chosen(set("yes")),
            Err(UnreadableAccounts {
                given: "yes".to_owned()
            }),
            "a server meant to require signing in must not start open because of a typo"
        );
    }

    #[test]
    fn a_refusal_says_what_was_given_and_what_would_do() {
        assert_eq!(
            UnreadableAccounts {
                given: "yes".to_owned()
            }
            .to_string(),
            "WEAVELING_ACCOUNTS is 'yes'; set it to on or off"
        );
    }
}
