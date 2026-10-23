use std::env;
use std::fmt::{self, Display, Formatter};

use accounts_contract::sign_up;
use thiserror::Error;

pub const WEAVELING_SIGN_UP: &str = "WEAVELING_SIGN_UP";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum SignUp {
    Open,
    ByInvitation,
}

#[derive(Debug, Error, PartialEq, Eq)]
#[error("{WEAVELING_SIGN_UP} is '{given}'; set it to open or invitation")]
pub struct UnreadableSignUp {
    pub given: String,
}

impl SignUp {
    pub fn chosen(given: Option<String>) -> Result<Self, UnreadableSignUp> {
        let Some(given) = given else {
            return Ok(Self::Open);
        };

        match given.trim().to_lowercase().as_str() {
            "open" | "" => Ok(Self::Open),
            "invitation" => Ok(Self::ByInvitation),
            _ => Err(UnreadableSignUp { given }),
        }
    }

    pub fn from_environment() -> Result<Self, UnreadableSignUp> {
        Self::chosen(env::var(WEAVELING_SIGN_UP).ok())
    }

    pub fn told(self) -> &'static str {
        match self {
            Self::Open => sign_up::OPEN,
            Self::ByInvitation => sign_up::BY_INVITATION,
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

    fn set(value: &str) -> Option<String> {
        Some(value.to_owned())
    }

    #[test]
    fn nothing_set_means_anyone_may_sign_up() {
        assert_eq!(SignUp::chosen(None), Ok(SignUp::Open));
        assert_eq!(SignUp::chosen(set("  ")), Ok(SignUp::Open));
    }

    #[test]
    fn open_and_invitation_are_read_however_they_are_written() {
        assert_eq!(SignUp::chosen(set("open")), Ok(SignUp::Open));
        assert_eq!(SignUp::chosen(set(" Open ")), Ok(SignUp::Open));
        assert_eq!(SignUp::chosen(set("invitation")), Ok(SignUp::ByInvitation));
        assert_eq!(SignUp::chosen(set("INVITATION")), Ok(SignUp::ByInvitation));
    }

    #[test]
    fn anything_else_is_refused_rather_than_guessed() {
        assert_eq!(
            SignUp::chosen(set("invited")),
            Err(UnreadableSignUp {
                given: "invited".to_owned()
            }),
            "a server meant to be closed must not start open because of a typo"
        );
        assert_eq!(
            UnreadableSignUp {
                given: "invited".to_owned()
            }
            .to_string(),
            "WEAVELING_SIGN_UP is 'invited'; set it to open or invitation"
        );
    }

    #[test]
    fn the_client_is_told_the_way_in_by_the_contracts_words() {
        assert_eq!(SignUp::Open.told(), "open");
        assert_eq!(SignUp::ByInvitation.told(), "invitation");
    }
}
