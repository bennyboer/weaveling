use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub const NOTED: &str = "notice.noted";
pub const DISMISSED: &str = "notice.dismissed";
pub const EVERY_NOTICE: &str = "notice.#";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "name")]
pub enum NoticeEventDTO {
    #[serde(rename = "NOTED")]
    Noted { account: String, about: AboutDTO },
    #[serde(rename = "DISMISSED")]
    Dismissed,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct NoticeDTO {
    pub id: String,
    pub noted_at: String,
    pub about: AboutDTO,
    #[serde(default, skip_serializing_if = "BTreeMap::is_empty")]
    pub names: BTreeMap<String, String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "kebab-case")]
pub enum AboutDTO {
    RoleChanged {
        project: String,
        role: String,
        #[serde(default, skip_serializing_if = "Option::is_none")]
        by: Option<String>,
    },
}

pub mod role {
    pub const OWNER: &str = "owner";
    pub const AUTHOR: &str = "author";
    pub const READER: &str = "reader";
}
