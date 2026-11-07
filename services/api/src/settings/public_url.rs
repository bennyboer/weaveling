use accounts_core::{InvalidPublicUrl, KeyCeremonies};
use thiserror::Error;

use super::{Setting, Settings};

pub const WEAVELING_PUBLIC_URL: &str = "WEAVELING_PUBLIC_URL";

#[derive(Debug, Error, PartialEq, Eq)]
#[error("{setting}, to which no key can be bound: {why}")]
pub struct UnreadablePublicUrl {
    pub setting: Setting,
    pub why: InvalidPublicUrl,
}

impl Settings {
    pub fn key_ceremonies(&self) -> Result<Option<KeyCeremonies>, UnreadablePublicUrl> {
        self.public_url
            .as_ref()
            .map(|setting| {
                KeyCeremonies::serving_at(&setting.value).map_err(|why| UnreadablePublicUrl {
                    setting: setting.clone(),
                    why,
                })
            })
            .transpose()
    }
}
