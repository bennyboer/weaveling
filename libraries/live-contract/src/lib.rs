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
