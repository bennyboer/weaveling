use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

pub const NO_LONGER_ADMITTED: u16 = 4403;

pub const ACCESS_CHANGED: &str = "access-changed";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct NoticeDTO {
    pub notice: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FrameDTO {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<String>,
    pub version: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    #[serde(default)]
    pub event: Value,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "do", rename_all = "kebab-case")]
pub enum AskedDTO {
    Subscribe {
        id: String,
        topic: String,
        #[serde(default)]
        about: BTreeMap<String, String>,
    },
    Unsubscribe {
        id: String,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "told", rename_all = "kebab-case")]
pub enum ToldDTO {
    Frame { id: String, frame: Value },
    Notice { id: String, notice: String },
    Ended { id: String, why: String },
}

pub mod ended {
    pub const UNKNOWN_TOPIC: &str = "unknown-topic";
    pub const NOT_THERE: &str = "not-there";
    pub const FAILED: &str = "failed";
    pub const ACCESS_LOST: &str = "access-lost";
    pub const BEHIND: &str = "behind";
}
