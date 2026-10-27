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

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct MemberDTO {
    pub account: String,
    pub role: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub name: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub email: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct ChangeRoleRequest {
    pub role: String,
}

pub mod refused {
    pub const NOT_AN_OWNER: &str = "not-an-owner";
    pub const LAST_OWNER: &str = "last-owner";
    pub const NOT_A_MEMBER: &str = "not-a-member";
    pub const ROLE_UNKNOWN: &str = "role-unknown";

    pub const EVERY: [&str; 4] = [NOT_AN_OWNER, LAST_OWNER, NOT_A_MEMBER, ROLE_UNKNOWN];
}

pub mod role {
    pub const OWNER: &str = "owner";
    pub const AUTHOR: &str = "author";
    pub const READER: &str = "reader";
}
