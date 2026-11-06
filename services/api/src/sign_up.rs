use std::fmt::{self, Display, Formatter};

use accounts_contract::sign_up;
use accounts_core::SigningUp;
use thiserror::Error;

use crate::settings::Setting;

pub const WEAVELING_SIGN_UP: &str = "WEAVELING_SIGN_UP";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignUp {
    Open,
    ByInvitation,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("{setting}; set it to open or invitation")]
pub struct UnreadableSignUp {
    pub setting: Setting,
}

impl SignUp {
    pub fn chosen(setting: Option<Setting>) -> Result<Self, UnreadableSignUp> {
        let Some(setting) = setting else {
            return Ok(Self::Open);
        };

        match setting.value.trim().to_lowercase().as_str() {
            "open" | "" => Ok(Self::Open),
            "invitation" => Ok(Self::ByInvitation),
            _ => Err(UnreadableSignUp { setting }),
        }
    }

    pub fn told(self) -> &'static str {
        match self {
            Self::Open => sign_up::OPEN,
            Self::ByInvitation => sign_up::BY_INVITATION,
        }
    }
}

impl From<SignUp> for SigningUp {
    fn from(sign_up: SignUp) -> Self {
        match sign_up {
            SignUp::Open => Self::Open,
            SignUp::ByInvitation => Self::ByInvitation,
        }
    }
}

impl Display for SignUp {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Open => write!(f, "anyone may sign up"),
            Self::ByInvitation => write!(f, "signing up by invitation only"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn set(value: &str) -> Option<Setting> {
        Some(Setting::new(WEAVELING_SIGN_UP, value))
    }

    #[test]
    fn nothing_set_means_anyone_may_sign_up() {
        // expect: nothing set lets anyone sign up
        assert_eq!(SignUp::chosen(None), Ok(SignUp::Open));
        assert_eq!(SignUp::chosen(set("  ")), Ok(SignUp::Open));
    }

    #[test]
    fn open_and_invitation_are_read_however_they_are_written() {
        // expect: open and invitation are read whatever their case and spacing
        assert_eq!(SignUp::chosen(set("open")), Ok(SignUp::Open));
        assert_eq!(SignUp::chosen(set(" Open ")), Ok(SignUp::Open));
        assert_eq!(SignUp::chosen(set("invitation")), Ok(SignUp::ByInvitation));
        assert_eq!(SignUp::chosen(set("INVITATION")), Ok(SignUp::ByInvitation));
    }

    #[test]
    fn anything_else_is_refused_rather_than_guessed() {
        // expect: any other setting is refused
        assert_eq!(
            SignUp::chosen(set("invited")),
            Err(UnreadableSignUp {
                setting: Setting::new(WEAVELING_SIGN_UP, "invited"),
            }),
            "a server meant to be closed must not start open because of a typo"
        );
        // and: the refusal names what was given and what would do
        assert_eq!(
            UnreadableSignUp {
                setting: Setting::new(WEAVELING_SIGN_UP, "invited"),
            }
            .to_string(),
            "WEAVELING_SIGN_UP is 'invited'; set it to open or invitation"
        );
    }

    #[test]
    fn the_client_is_told_the_way_in_by_the_contracts_words() {
        // expect: each way in is told by the contract's word
        assert_eq!(SignUp::Open.told(), "open");
        assert_eq!(SignUp::ByInvitation.told(), "invitation");
    }
}
