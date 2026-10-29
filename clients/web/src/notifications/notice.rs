use crate::invitations::to_you::InvitationToYou;

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Notice {
    Invitation(InvitationToYou),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Group {
    Invitations,
}

impl Group {
    pub const ALL: [Group; 1] = [Group::Invitations];

    pub fn message(self) -> &'static str {
        match self {
            Self::Invitations => "notifications-invitations",
        }
    }
}

impl Notice {
    pub fn group(&self) -> Group {
        match self {
            Self::Invitation(_) => Group::Invitations,
        }
    }
}
