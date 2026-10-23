use serde::{Deserialize, Serialize};

pub const ISSUED: &str = "invitation.issued";
pub const ACCEPTED: &str = "invitation.accepted";
pub const WITHDRAWN: &str = "invitation.withdrawn";
pub const EVERY_INVITATION: &str = "invitation.#";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "name")]
pub enum InvitationEventDTO {
    #[serde(rename = "ISSUED")]
    Issued,
    #[serde(rename = "ACCEPTED")]
    Accepted,
    #[serde(rename = "WITHDRAWN")]
    Withdrawn,
}
