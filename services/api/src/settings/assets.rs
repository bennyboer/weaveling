use std::path::PathBuf;

use assets_wiring::{Keeping, Limits};
use thiserror::Error;
use units::UnreadableSize;

use super::{Setting, Settings};
use crate::Accounts;

pub const WEAVELING_ASSETS_MOST_EACH_UPLOAD: &str = "WEAVELING_ASSETS_MOST_EACH_UPLOAD";
pub const WEAVELING_ASSETS_MOST_KEPT: &str = "WEAVELING_ASSETS_MOST_KEPT";
pub const WEAVELING_ASSETS_MOST_EACH_ACCOUNT: &str = "WEAVELING_ASSETS_MOST_EACH_ACCOUNT";
pub const WEAVELING_ASSETS_MOST_EACH_PROJECT: &str = "WEAVELING_ASSETS_MOST_EACH_PROJECT";
pub const WEAVELING_ASSETS_FOLDER: &str = "WEAVELING_ASSETS_FOLDER";

const MEGABYTE: u64 = 1024 * 1024;
const MOST_EACH_UPLOAD_UNLESS_SET: u64 = 20 * MEGABYTE;
const MOST_EACH_ACCOUNT_UNLESS_SET: u64 = 1024 * MEGABYTE;

#[derive(Debug, Error, PartialEq, Eq)]
#[error("{setting}; {why}")]
pub struct UnreadableAssets {
    pub setting: Setting,
    pub why: UnfitSize,
}

#[derive(Debug, Error, PartialEq, Eq)]
pub enum UnfitSize {
    #[error(transparent)]
    Unreadable(#[from] UnreadableSize),
    #[error("nought; set more than nothing, or leave it out")]
    Nought,
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

impl Default for AssetSettings {
    fn default() -> Self {
        Self {
            most_each_upload: MOST_EACH_UPLOAD_UNLESS_SET,
            most_kept: None,
            most_each_account: MOST_EACH_ACCOUNT_UNLESS_SET,
            most_each_project: None,
            folder: None,
        }
    }
}

impl AssetSettings {
    /// What the assets are kept to: an account held to its limit only where there are accounts.
    pub fn keeping(&self, accounts: Accounts) -> Keeping {
        Keeping {
            limits: Limits {
                each_upload: Some(self.most_each_upload),
                instance: self.most_kept,
                each_account: match accounts {
                    Accounts::On => Some(self.most_each_account),
                    Accounts::Off => None,
                },
                each_project: self.most_each_project,
            },
            folder: self.folder.clone(),
        }
    }
}

impl Settings {
    pub fn assets(&self) -> Result<AssetSettings, UnreadableAssets> {
        let unless_set = AssetSettings::default();

        Ok(AssetSettings {
            most_each_upload: size(self.assets_most_each_upload.as_ref())?
                .unwrap_or(unless_set.most_each_upload),
            most_kept: size(self.assets_most_kept.as_ref())?,
            most_each_account: size(self.assets_most_each_account.as_ref())?
                .unwrap_or(unless_set.most_each_account),
            most_each_project: size(self.assets_most_each_project.as_ref())?,
            folder: self
                .assets_folder
                .as_ref()
                .map(|setting| PathBuf::from(setting.value.trim())),
        })
    }
}

fn size(setting: Option<&Setting>) -> Result<Option<u64>, UnreadableAssets> {
    let Some(setting) = setting else {
        return Ok(None);
    };

    let refused = |why| UnreadableAssets {
        setting: setting.clone(),
        why,
    };
    match units::bytes(&setting.value) {
        Ok(0) => Err(refused(UnfitSize::Nought)),
        Ok(most) => Ok(Some(most)),
        Err(unreadable) => Err(refused(UnfitSize::from(unreadable))),
    }
}
