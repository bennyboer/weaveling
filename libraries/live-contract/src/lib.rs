use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A notice told to a follower still let in after who may follow changed, as its role may be another now.
pub const ACCESS_CHANGED: &str = "access-changed";

/// The code a socket is closed with when its session ends; the client then asks no more and signs out.
pub const SESSION_ENDED: u16 = 4401;

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct FrameDTO {
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub of: Option<String>,
    pub version: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub at: Option<String>,
    #[serde(default, skip_serializing_if = "Value::is_null")]
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
    Frame { id: String, frame: FrameDTO },
    Notice { id: String, notice: String },
    Ended { id: String, why: String },
}

/// Why a subscription ended, told under its id; the socket and its other subscriptions go on.
pub mod ended {
    pub const UNKNOWN_TOPIC: &str = "unknown-topic";
    pub const NOT_THERE: &str = "not-there";
    pub const FAILED: &str = "failed";
    pub const ACCESS_LOST: &str = "access-lost";
    /// The follower fell too far behind to be told every frame; asked again, it follows from where it is.
    pub const BEHIND: &str = "behind";
}
