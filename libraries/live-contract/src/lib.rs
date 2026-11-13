use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

/// A notice sent to a follower that is still let in after access changed.
/// Its role may be different now.
pub const ACCESS_CHANGED: &str = "access-changed";

/// The code a socket is closed with when its session ends.
/// The client then stops asking and signs out.
pub const SESSION_ENDED: u16 = 4401;

/// The code a socket is closed with when the server is stopping.
/// The client opens the socket again once the server is back.
pub const SERVER_CLOSING: u16 = 1001;

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
    /// The subscription receives its channel from now on.
    /// So anything read after this misses no later change.
    Following {
        id: String,
    },
    Frame {
        id: String,
        frame: FrameDTO,
    },
    Notice {
        id: String,
        notice: String,
    },
    Ended {
        id: String,
        why: String,
    },
}

/// Why a subscription ended, sent with its id.
/// The socket and its other subscriptions keep going.
pub mod ended {
    pub const UNKNOWN_TOPIC: &str = "unknown-topic";
    pub const NOT_THERE: &str = "not-there";
    pub const FAILED: &str = "failed";
    pub const ACCESS_LOST: &str = "access-lost";
    /// The follower fell too far behind to receive every frame.
    /// If it subscribes again, it follows from that point on.
    pub const BEHIND: &str = "behind";
}
