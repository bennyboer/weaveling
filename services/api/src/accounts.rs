use std::fmt::{self, Display, Formatter};

use thiserror::Error;

use crate::settings::Setting;

pub const WEAVELING_ACCOUNTS: &str = "WEAVELING_ACCOUNTS";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Accounts {
    On,
    Off,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("{setting}; set it to on or off")]
pub struct UnreadableAccounts {
    pub setting: Setting,
}

impl Accounts {
    pub fn chosen(setting: Option<Setting>) -> Result<Self, UnreadableAccounts> {
        let Some(setting) = setting else {
            return Ok(Self::Off);
        };

        match setting.value.trim().to_lowercase().as_str() {
            "on" => Ok(Self::On),
            "off" | "" => Ok(Self::Off),
            _ => Err(UnreadableAccounts { setting }),
        }
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

    fn set(value: &str) -> Option<Setting> {
        Some(Setting::new(WEAVELING_ACCOUNTS, value))
    }

    #[test]
    fn nothing_set_means_no_accounts() {
        // expect: nothing set leaves accounts off
        assert_eq!(
            Accounts::chosen(None),
            Ok(Accounts::Off),
            "local mode is what needs no setting, and must never show accounts by accident"
        );
    }

    #[test]
    fn on_and_off_are_read_however_they_are_written() {
        // expect: on and off are read whatever their case and spacing
        assert_eq!(Accounts::chosen(set("on")), Ok(Accounts::On));
        assert_eq!(Accounts::chosen(set(" ON ")), Ok(Accounts::On));
        assert_eq!(Accounts::chosen(set("off")), Ok(Accounts::Off));
        assert_eq!(Accounts::chosen(set("Off")), Ok(Accounts::Off));
    }

    #[test]
    fn a_blank_setting_counts_as_unset() {
        // expect: a blank setting leaves accounts off
        assert_eq!(Accounts::chosen(set("  ")), Ok(Accounts::Off));
    }

    #[test]
    fn anything_else_is_refused_rather_than_guessed() {
        // expect: any other setting is refused
        assert_eq!(
            Accounts::chosen(set("yes")),
            Err(UnreadableAccounts {
                setting: Setting::new(WEAVELING_ACCOUNTS, "yes"),
            }),
            "a server meant to require signing in must not start open because of a typo"
        );
    }

    #[test]
    fn a_refusal_says_what_was_given_and_what_would_do() {
        // expect: a refusal names what was given and what would do
        assert_eq!(
            UnreadableAccounts {
                setting: Setting::new(WEAVELING_ACCOUNTS, "yes"),
            }
            .to_string(),
            "WEAVELING_ACCOUNTS is 'yes'; set it to on or off"
        );
    }
}
