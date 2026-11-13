use std::fs;

use accounts_core::TotpKey;
use thiserror::Error;

use super::{Setting, Settings, Unsettled};

pub const WEAVELING_TOTP_KEY: &str = "WEAVELING_TOTP_KEY";
pub const WEAVELING_TOTP_KEY_FILE: &str = "WEAVELING_TOTP_KEY_FILE";

#[derive(Debug, Error, PartialEq, Eq)]
#[error(
    "{0} is no TOTP key; set it to 64 hexadecimal digits, such as `openssl rand -hex 32` prints"
)]
pub struct UnreadableTotpKey(pub String);

impl Settings {
    pub fn totp_key(&self) -> Result<Option<TotpKey>, UnreadableTotpKey> {
        self.totp_key
            .as_ref()
            .map(|setting| {
                TotpKey::from_hex(&setting.value)
                    .map_err(|_| UnreadableTotpKey(setting.source.clone()))
            })
            .transpose()
    }
}

/// A secret, given either directly or as the path of a file that holds it.
/// This way the secret need not be put in a variable or in the settings file.
pub(super) fn secret(
    given: Option<Setting>,
    in_file: Option<Setting>,
) -> Result<Option<Setting>, Unsettled> {
    match (given, in_file) {
        (Some(given), Some(in_file)) => Err(Unsettled::Doubled(given.source, in_file.source)),
        (Some(given), None) => Ok(Some(Setting::secret(given.source, given.value))),
        (None, Some(in_file)) => {
            let path = in_file.value.trim();
            let read = fs::read_to_string(path).map_err(|why| {
                Unsettled::SecretFile(in_file.source.clone(), path.into(), why.to_string())
            })?;

            Ok(Some(Setting::secret(
                format!("the file {path} named by {}", in_file.source),
                read.trim(),
            )))
        }
        (None, None) => Ok(None),
    }
}
