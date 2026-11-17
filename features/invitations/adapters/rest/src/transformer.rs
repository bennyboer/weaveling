use clock::text;
use invitations_contract::{
    InvitationSummaryDTO, InvitationToYouDTO, ScopeDTO, decline_reason, role, status,
};
use invitations_core::{DeclineReason, InvitationToYou, ListedInvitation, Role, Scope, Status};

pub(crate) fn to_you_dto(yours: &InvitationToYou) -> InvitationToYouDTO {
    InvitationToYouDTO {
        id: yours.id.to_string(),
        project: yours.project.to_string(),
        project_name: yours.project_name.clone(),
        role: named(yours.role).to_owned(),
        invited_by: yours.invited_by.clone(),
        until: text::serialize(yours.until),
    }
}

pub(crate) fn to_summary_dto(listed: &ListedInvitation) -> InvitationSummaryDTO {
    let (told, declined_for) = match listed.status {
        Status::Open => (status::OPEN, None),
        Status::Expired => (status::EXPIRED, None),
        Status::Declined(DeclineReason::AddressHasAccount) => (
            status::DECLINED,
            Some(decline_reason::ADDRESS_HAS_ACCOUNT.to_owned()),
        ),
        Status::Declined(DeclineReason::TurnedDown) => (
            status::DECLINED,
            Some(decline_reason::TURNED_DOWN.to_owned()),
        ),
    };

    InvitationSummaryDTO {
        id: listed.summary.id.to_string(),
        version: listed.summary.version.count(),
        email: listed.summary.to.to_string(),
        issued_at: text::serialize(listed.summary.issued_at),
        until: text::serialize(listed.summary.until),
        status: told.to_owned(),
        declined_for,
        scope: match &listed.summary.scope {
            Scope::Instance => ScopeDTO::Instance,
            Scope::Project { project, role } => ScopeDTO::Project {
                project: project.to_string(),
                role: named(*role).to_owned(),
            },
        },
    }
}

fn named(given: Role) -> &'static str {
    match given {
        Role::Owner => role::OWNER,
        Role::Author => role::AUTHOR,
        Role::Reader => role::READER,
    }
}

pub(crate) fn to_role(named: &str) -> Option<Role> {
    match named {
        role::OWNER => Some(Role::Owner),
        role::AUTHOR => Some(Role::Author),
        role::READER => Some(Role::Reader),
        _ => None,
    }
}
