use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;
use std::sync::Arc;

use async_trait::async_trait;
use mailing::{DroppingMailer, LoggingMailer, Mail, Mailer, Unsent};
use thiserror::Error;

use crate::Accounts;
use crate::settings::Setting;

pub const WEAVELING_MAIL_DROP: &str = "WEAVELING_MAIL_DROP";
pub const WEAVELING_MAIL_LOG: &str = "WEAVELING_MAIL_LOG";

/// Where mail goes until there is a mail server to send it.
/// Each mail is written as a file into a folder, written into the log when chosen for development, or sent nowhere.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mailing {
    Logged,
    Dropped { into: PathBuf },
    Nowhere,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum Unmailable {
    #[error("{setting}; set it to on or off")]
    Unreadable { setting: Setting },
    #[error(
        "accounts are on, but no mail would reach anyone: set the mail drop ([mail] drop, {WEAVELING_MAIL_DROP}), or, while developing, write mails to the log ([mail] log = true, {WEAVELING_MAIL_LOG}=on), codes and links in them; see weaveling.example.toml"
    )]
    Nowhere,
}

impl Mailing {
    pub fn chosen(drop: Option<Setting>, log: Option<Setting>) -> Result<Self, Unmailable> {
        if let Some(into) = drop.filter(|drop| !drop.value.trim().is_empty()) {
            return Ok(Self::Dropped {
                into: into.value.into(),
            });
        }
        let Some(log) = log else {
            return Ok(Self::Nowhere);
        };

        match log.value.trim().to_lowercase().as_str() {
            "on" => Ok(Self::Logged),
            "off" | "" => Ok(Self::Nowhere),
            _ => Err(Unmailable::Unreadable { setting: log }),
        }
    }

    /// The mailing a server starts with, depending on whether it has accounts.
    /// A server with accounts needs its mail to reach someone.
    pub fn ready_for(self, accounts: Accounts) -> Result<Self, Unmailable> {
        match (accounts, &self) {
            (Accounts::On, Self::Nowhere) => Err(Unmailable::Nowhere),
            _ => Ok(self),
        }
    }

    pub fn mailer(&self) -> Arc<dyn Mailer> {
        match self {
            Self::Logged => Arc::new(LoggingMailer),
            Self::Dropped { into } => Arc::new(DroppingMailer::into(into)),
            Self::Nowhere => Arc::new(Unmailed),
        }
    }
}

impl Display for Mailing {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Logged => write!(f, "mails written to the log"),
            Self::Dropped { into } => write!(f, "mails dropped into {}", into.display()),
            Self::Nowhere => write!(f, "no mails sent"),
        }
    }
}

/// Sends nothing and refuses each mail.
/// It is used when mail is not set up to go anywhere.
struct Unmailed;

#[async_trait]
impl Mailer for Unmailed {
    async fn send(&self, mail: Mail) -> Result<(), Unsent> {
        Err(Unsent {
            to: mail.to,
            because: "nothing is set for mail to go anywhere".to_owned(),
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn drop(value: &str) -> Option<Setting> {
        Some(Setting::new(WEAVELING_MAIL_DROP, value))
    }

    fn log(value: &str) -> Option<Setting> {
        Some(Setting::new(WEAVELING_MAIL_LOG, value))
    }

    #[test]
    fn nothing_set_sends_mail_nowhere() {
        // expect: nothing set, or a drop left blank, sends mail nowhere
        assert_eq!(Mailing::chosen(None, None), Ok(Mailing::Nowhere));
        assert_eq!(Mailing::chosen(drop("  "), None), Ok(Mailing::Nowhere));
        // and: so does the log turned off
        assert_eq!(Mailing::chosen(None, log("off")), Ok(Mailing::Nowhere));
    }

    #[test]
    fn a_drop_named_keeps_each_mail_there_whatever_the_log() {
        // expect: a named drop keeps each mail there
        for logged in [None, log("on")] {
            assert_eq!(
                Mailing::chosen(drop("target/mails"), logged),
                Ok(Mailing::Dropped {
                    into: PathBuf::from("target/mails")
                })
            );
        }
    }

    #[test]
    fn mail_goes_to_the_log_only_when_chosen_so() {
        // expect: the log turned on writes mail to it
        assert_eq!(Mailing::chosen(None, log(" On ")), Ok(Mailing::Logged));
        // and: anything else is refused, saying where it was set
        assert_eq!(
            Mailing::chosen(None, log("yes")),
            Err(Unmailable::Unreadable {
                setting: Setting::new(WEAVELING_MAIL_LOG, "yes")
            })
        );
    }

    #[test]
    fn a_server_with_accounts_starts_only_with_mail_reaching_someone() {
        // expect: with accounts, mail going nowhere is refused
        assert_eq!(
            Mailing::Nowhere.ready_for(Accounts::On),
            Err(Unmailable::Nowhere)
        );
        // and: the log or a drop is fine
        assert_eq!(Mailing::Logged.ready_for(Accounts::On), Ok(Mailing::Logged));
        // and: without accounts, nothing needs to be sent
        assert_eq!(
            Mailing::Nowhere.ready_for(Accounts::Off),
            Ok(Mailing::Nowhere)
        );
    }
}
