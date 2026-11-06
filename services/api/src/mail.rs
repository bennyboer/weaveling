use std::fmt::{self, Display, Formatter};
use std::path::PathBuf;
use std::sync::Arc;

use mailing::{DroppingMailer, LoggingMailer, Mailer};

use crate::settings::Setting;

pub const WEAVELING_MAIL_DROP: &str = "WEAVELING_MAIL_DROP";

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Mailing {
    Logged,
    Dropped { into: PathBuf },
}

impl Mailing {
    pub fn chosen(drop: Option<Setting>) -> Self {
        match drop.filter(|drop| !drop.value.trim().is_empty()) {
            Some(into) => Self::Dropped {
                into: into.value.into(),
            },
            None => Self::Logged,
        }
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
        // expect: nothing set writes mails to the log
        assert_eq!(Mailing::chosen(None), Mailing::Logged);
        assert_eq!(
            Mailing::chosen(Some(Setting::new(WEAVELING_MAIL_DROP, "  "))),
            Mailing::Logged
        );
    }

    #[test]
    fn a_drop_named_keeps_each_mail_there() {
        // expect: a named drop keeps each mail there
        assert_eq!(
            Mailing::chosen(Some(Setting::new(WEAVELING_MAIL_DROP, "target/mails"))),
            Mailing::Dropped {
                into: PathBuf::from("target/mails")
            }
        );
    }
}
