use std::env;
use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;
use std::sync::Arc;

use mailing::{DroppingMailer, LoggingMailer, Mailer};

pub const WEAVELING_MAIL_DROP: &str = "WEAVELING_MAIL_DROP";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mailing {
    Logged,
    Dropped { into: PathBuf },
}

impl Mailing {
    pub fn chosen(drop: Option<String>) -> Self {
        match drop.filter(|drop| !drop.trim().is_empty()) {
            Some(into) => Self::Dropped { into: into.into() },
            None => Self::Logged,
        }
    }

    pub fn from_environment() -> Self {
        Self::chosen(env::var(WEAVELING_MAIL_DROP).ok())
    }

    pub fn mailer(&self) -> Arc<dyn Mailer> {
        match self {
            Self::Logged => Arc::new(LoggingMailer),
            Self::Dropped { into } => Arc::new(DroppingMailer::into(into)),
        }
    }
}

impl Display for Mailing {
    fn fmt(&self, f: &mut Formatter<'_>) -> fmt::Result {
        match self {
            Self::Logged => write!(f, "mails written to the log"),
            Self::Dropped { into } => write!(f, "mails dropped into {}", into.display()),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn nothing_set_writes_mails_to_the_log() {
        assert_eq!(Mailing::chosen(None), Mailing::Logged);
        assert_eq!(Mailing::chosen(Some("  ".to_owned())), Mailing::Logged);
    }

    #[test]
    fn a_drop_named_keeps_each_mail_there() {
        assert_eq!(
            Mailing::chosen(Some("target/mails".to_owned())),
            Mailing::Dropped {
                into: PathBuf::from("target/mails")
            }
        );
    }
}
