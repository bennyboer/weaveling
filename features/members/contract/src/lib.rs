use serde::{Deserialize, Serialize};

pub const FOUNDED: &str = "members.founded";
pub const JOINED: &str = "members.joined";
pub const ROLE_CHANGED: &str = "members.role.changed";
pub const LEFT: &str = "members.left";
pub const REMOVED: &str = "members.removed";
pub const EVERY_MEMBERS: &str = "members.#";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "name")]
pub enum MembersEventDTO {
    #[serde(rename = "FOUNDED")]
    Founded { owner: String },
    #[serde(rename = "JOINED")]
    Joined { account: String, role: String },
    #[serde(rename = "ROLE_CHANGED")]
    RoleChanged { account: String, to: String },
    #[serde(rename = "LEFT")]
    Left { account: String },
    #[serde(rename = "REMOVED")]
    Removed { account: String },
}

pub mod role {
    pub const OWNER: &str = "owner";
    pub const AUTHOR: &str = "author";
    pub const READER: &str = "reader";
}
