use axum::http::uri::Authority;
use thiserror::Error;

use super::{Setting, Settings};

pub const WEAVELING_HOSTS: &str = "WEAVELING_HOSTS";

#[derive(Debug, Error, PartialEq, Eq)]
#[error(
    "{setting}, where '{host}' is no host name; name each alone, such as mybox.local, without a scheme or a port"
)]
pub struct UnreadableHosts {
    pub setting: Setting,
    pub host: String,
}

impl Settings {
    /// The names the instance is opened by besides localhost, a plain address and its public URL: a home network's, say.
    pub fn hosts(&self) -> Result<Vec<String>, UnreadableHosts> {
        let Some(setting) = &self.hosts else {
            return Ok(Vec::new());
        };

        setting
            .value
            .split(',')
            .map(str::trim)
            .filter(|host| !host.is_empty())
            .map(|host| match host.parse::<Authority>() {
                Ok(authority) if authority.as_str() == authority.host() => {
                    Ok(host.to_ascii_lowercase())
                }
                _ => Err(UnreadableHosts {
                    setting: setting.clone(),
                    host: host.to_owned(),
                }),
            })
            .collect()
    }
}
