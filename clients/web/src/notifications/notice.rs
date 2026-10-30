use time::OffsetDateTime;

use crate::invitations::to_you::InvitationToYou;
use crate::members::listed::Role;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RoleChange {
    pub id: String,
    pub noted_at: OffsetDateTime,
    pub role: Role,
    pub project_name: Option<String>,
    pub by_name: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    Invitation(InvitationToYou),
    RoleChanged(RoleChange),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Invitations,
    Roles,
}

impl Group {
    pub const ALL: [Group; 2] = [Group::Invitations, Group::Roles];

    pub fn message(self) -> &'static str {
        match self {
            Self::Invitations => "notifications-invitations",
            Self::Roles => "notifications-roles",
        }
    }
}

impl Notice {
    pub fn group(&self) -> Group {
        match self {
            Self::Invitation(_) => Group::Invitations,
            Self::RoleChanged(_) => Group::Roles,
        }
    }
}
