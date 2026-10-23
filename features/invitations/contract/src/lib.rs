use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct InviteRequest {
    pub email: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
pub struct InvitedDTO {
    pub id: String,
}

pub const ISSUED: &str = "invitation.issued";
pub const ACCEPTED: &str = "invitation.accepted";
pub const WITHDRAWN: &str = "invitation.withdrawn";
pub const DECLINED: &str = "invitation.declined";
pub const EVERY_INVITATION: &str = "invitation.#";

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "name")]
pub enum InvitationEventDTO {
    #[serde(rename = "ISSUED")]
    Issued {
        to: String,
        digest: String,
        until: String,
    },
    #[serde(rename = "ACCEPTED")]
    Accepted,
    #[serde(rename = "WITHDRAWN")]
    Withdrawn,
    #[serde(rename = "DECLINED")]
    Declined,
}

pub mod refused {
    pub const NOT_AN_OPERATOR: &str = "not-an-operator";
    pub const EMAIL_MALFORMED: &str = "email-malformed";
    pub const EMAIL_TOO_LONG: &str = "email-too-long";
    pub const THROTTLED: &str = "too-many-tries";
    pub const ORIGIN_MISSING: &str = "origin-missing";
    pub const INVITATION_ID_INVALID: &str = "invitation-id-invalid";
    pub const INVITATION_NOT_FOUND: &str = "invitation-not-found";
    pub const INVITATION_ALREADY_ACCEPTED: &str = "invitation-already-accepted";

    pub const EVERY: [&str; 8] = [
        NOT_AN_OPERATOR,
        EMAIL_MALFORMED,
        EMAIL_TOO_LONG,
        THROTTLED,
        ORIGIN_MISSING,
        INVITATION_ID_INVALID,
        INVITATION_NOT_FOUND,
        INVITATION_ALREADY_ACCEPTED,
    ];
}
