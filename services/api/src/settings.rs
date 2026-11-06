mod file;
mod listen;
mod setting;
#[cfg(test)]
mod tests;
mod totp_key;

use std::env;
use std::path::{Path, PathBuf};

use thiserror::Error;

use crate::{
    Accounts, Backend, Flakiness, Mailing, SignUp, Unchosen, Unflaky, UnreadableAccounts,
    UnreadableSignUp, WEAVELING_ACCOUNTS, WEAVELING_DATA, WEAVELING_DATABASE_URL, WEAVELING_FLAKY,
    WEAVELING_MAIL_DROP, WEAVELING_SIGN_UP,
};
use file::BESIDE;

pub use listen::{Unlistenable, WEAVELING_LISTEN};
pub use setting::Setting;
pub use totp_key::{UnreadableTotpKey, WEAVELING_TOTP_KEY, WEAVELING_TOTP_KEY_FILE};

pub const WEAVELING_CONFIG: &str = "WEAVELING_CONFIG";

#[derive(Debug, Error)]
pub enum Unsettled {
    #[error("{0} names no settings file there is: {1}")]
    NoFile(String, PathBuf),
    #[error("the settings file {0} cannot be read: {1}")]
    Unreadable(PathBuf, String),
    #[error("{0} and {1} are both set; set one of them")]
    Doubled(String, String),
    #[error("{0} names the file {1}, which cannot be read: {2}")]
    SecretFile(String, PathBuf, String),
    #[error(transparent)]
    Backend(#[from] Unchosen),
    #[error(transparent)]
    Accounts(#[from] UnreadableAccounts),
    #[error(transparent)]
    SignUp(#[from] UnreadableSignUp),
    #[error(transparent)]
    Flaky(#[from] Unflaky),
    #[error(transparent)]
    Listen(#[from] Unlistenable),
    #[error(transparent)]
    TotpKey(#[from] UnreadableTotpKey),
}

/// Every setting the server starts with, read once: built-in defaults, then a settings file, then the variables, the later winning.
#[derive(Debug, Default)]
pub struct Settings {
    listen: Option<Setting>,
    database_url: Option<Setting>,
    data: Option<Setting>,
    accounts: Option<Setting>,
    sign_up: Option<Setting>,
    mail_drop: Option<Setting>,
    flaky: Option<Setting>,
    totp_key: Option<Setting>,
}

impl Settings {
    /// The settings this process was started with: its arguments and variables, and the file they name or the one beside it.
    pub fn of_this_process() -> Result<Self, Unsettled> {
        Self::read(
            env::args().skip(1),
            |name| env::var(name).ok(),
            Path::new("."),
        )
    }

    pub fn read(
        arguments: impl IntoIterator<Item = String>,
        variable: impl Fn(&str) -> Option<String>,
        beside: &Path,
    ) -> Result<Self, Unsettled> {
        let mut settings = match file::named(arguments, &variable) {
            Some((named_by, path)) if !path.is_file() => {
                return Err(Unsettled::NoFile(named_by, path));
            }
            Some((_, path)) => Self::filed(&path)?,
            None if beside.join(BESIDE).is_file() => Self::filed(&beside.join(BESIDE))?,
            None => Self::default(),
        };
        settings.set_by_variables(&variable)?;

        Ok(settings)
    }

    pub fn backend(&self) -> Result<Backend, Unchosen> {
        Backend::chosen(self.database_url.clone(), self.data.clone())
    }

    pub fn accounts(&self) -> Result<Accounts, UnreadableAccounts> {
        Accounts::chosen(self.accounts.clone())
    }

    pub fn sign_up(&self) -> Result<SignUp, UnreadableSignUp> {
        SignUp::chosen(self.sign_up.clone())
    }

    pub fn mailing(&self) -> Mailing {
        Mailing::chosen(self.mail_drop.clone())
    }

    pub fn flakiness(&self) -> Result<Option<Flakiness>, Unflaky> {
        self.flaky.clone().map(Flakiness::parse).transpose()
    }

    fn set_by_variables(
        &mut self,
        variable: &impl Fn(&str) -> Option<String>,
    ) -> Result<(), Unsettled> {
        let set = |name: &str| {
            variable(name)
                .filter(|value| !value.trim().is_empty())
                .map(|value| Setting::new(name, value))
        };

        for (name, setting) in [
            (WEAVELING_LISTEN, &mut self.listen),
            (WEAVELING_DATABASE_URL, &mut self.database_url),
            (WEAVELING_DATA, &mut self.data),
            (WEAVELING_ACCOUNTS, &mut self.accounts),
            (WEAVELING_SIGN_UP, &mut self.sign_up),
            (WEAVELING_MAIL_DROP, &mut self.mail_drop),
            (WEAVELING_FLAKY, &mut self.flaky),
        ] {
            if let Some(given) = set(name) {
                *setting = Some(given);
            }
        }
        if let Some(totp_key) =
            totp_key::secret(set(WEAVELING_TOTP_KEY), set(WEAVELING_TOTP_KEY_FILE))?
        {
            self.totp_key = Some(totp_key);
        }

        Ok(())
    }
}
