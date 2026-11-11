use accounts_core::MAIL_EACH_HOUR;
use invitations_core::INVITATIONS_EACH_HOUR;
use thiserror::Error;

use super::{Setting, Settings};

pub const WEAVELING_MAIL_ANYONE_EACH_HOUR: &str = "WEAVELING_MAIL_ANYONE_EACH_HOUR";
pub const WEAVELING_MAIL_INVITATIONS_EACH_HOUR: &str = "WEAVELING_MAIL_INVITATIONS_EACH_HOUR";

#[derive(Debug, Error, PartialEq, Eq)]
#[error("{setting}; set it to how many mails each hour, a whole number above nought")]
pub struct UnreadableMailCap {
    pub setting: Setting,
}

/// How much mail the instance sends at most, so nobody mails strangers in bulk through it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct MailCaps {
    /// Codes and word of an address having an account, which anyone may have sent, across the instance.
    pub anyone_each_hour: u32,
    /// Invitations, by the account inviting.
    pub invitations_each_hour: u32,
}

impl Default for MailCaps {
    fn default() -> Self {
        Self {
            anyone_each_hour: MAIL_EACH_HOUR,
            invitations_each_hour: INVITATIONS_EACH_HOUR,
        }
    }
}

impl Settings {
    pub fn mail_caps(&self) -> Result<MailCaps, UnreadableMailCap> {
        Ok(MailCaps {
            anyone_each_hour: each_hour(self.mail_anyone_each_hour.as_ref(), MAIL_EACH_HOUR)?,
            invitations_each_hour: each_hour(
                self.mail_invitations_each_hour.as_ref(),
                INVITATIONS_EACH_HOUR,
            )?,
        })
    }
}

fn each_hour(setting: Option<&Setting>, unless_set: u32) -> Result<u32, UnreadableMailCap> {
    let Some(setting) = setting else {
        return Ok(unless_set);
    };

    match setting.value.trim().parse::<u32>() {
        Ok(most) if most > 0 => Ok(most),
        _ => Err(UnreadableMailCap {
            setting: setting.clone(),
        }),
    }
}
