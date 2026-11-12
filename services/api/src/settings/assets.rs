use std::path::PathBuf;

use thiserror::Error;

use super::{Setting, Settings};

pub const WEAVELING_ASSETS_MOST_EACH_UPLOAD_MB: &str = "WEAVELING_ASSETS_MOST_EACH_UPLOAD_MB";
pub const WEAVELING_ASSETS_MOST_KEPT_MB: &str = "WEAVELING_ASSETS_MOST_KEPT_MB";
pub const WEAVELING_ASSETS_MOST_EACH_ACCOUNT_MB: &str = "WEAVELING_ASSETS_MOST_EACH_ACCOUNT_MB";
pub const WEAVELING_ASSETS_MOST_EACH_PROJECT_MB: &str = "WEAVELING_ASSETS_MOST_EACH_PROJECT_MB";
pub const WEAVELING_ASSETS_FOLDER: &str = "WEAVELING_ASSETS_FOLDER";

const MEGABYTE: u64 = 1024 * 1024;
const MOST_EACH_UPLOAD_MB_UNLESS_SET: u64 = 20;
const MOST_EACH_ACCOUNT_MB_UNLESS_SET: u64 = 1024;

#[derive(Debug, Error, PartialEq, Eq)]
#[error("{setting}; set it to how many megabytes, a whole number above nought")]
pub struct UnreadableAssets {
    pub setting: Setting,
}

/// How much the instance takes for images and other assets, and where it keeps them.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct AssetSettings {
    /// The most one upload may be, in bytes.
    pub most_each_upload: u64,
    /// The most the instance keeps for every asset together, in bytes, none unless set.
    pub most_kept: Option<u64>,
    /// The most an account's uploads may take together, in bytes, wherever they went; held only where there are accounts.
    pub most_each_account: u64,
    /// The most a project's assets may take together, in bytes, none unless set.
    pub most_each_project: Option<u64>,
    /// A folder of the operator's choosing, in place of the database or the local data's own.
    pub folder: Option<PathBuf>,
}

impl Settings {
    pub fn assets(&self) -> Result<AssetSettings, UnreadableAssets> {
        Ok(AssetSettings {
            most_each_upload: megabytes(self.assets_most_each_upload_mb.as_ref())?
                .unwrap_or(MOST_EACH_UPLOAD_MB_UNLESS_SET * MEGABYTE),
            most_kept: megabytes(self.assets_most_kept_mb.as_ref())?,
            most_each_account: megabytes(self.assets_most_each_account_mb.as_ref())?
                .unwrap_or(MOST_EACH_ACCOUNT_MB_UNLESS_SET * MEGABYTE),
            most_each_project: megabytes(self.assets_most_each_project_mb.as_ref())?,
            folder: self
                .assets_folder
                .as_ref()
                .map(|setting| PathBuf::from(setting.value.trim())),
        })
    }
}

fn megabytes(setting: Option<&Setting>) -> Result<Option<u64>, UnreadableAssets> {
    let Some(setting) = setting else {
        return Ok(None);
    };

    match setting.value.trim().parse::<u64>() {
        Ok(most) if most > 0 => most
            .checked_mul(MEGABYTE)
            .map(Some)
            .ok_or(UnreadableAssets {
                setting: setting.clone(),
            }),
        _ => Err(UnreadableAssets {
            setting: setting.clone(),
        }),
    }
}
