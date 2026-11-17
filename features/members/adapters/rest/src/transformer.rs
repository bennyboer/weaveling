use clock::text;
use members_contract::{MemberDTO, MembershipDTO, role};
use members_core::{ListedMember, Membership, Role};

pub(crate) fn to_dto(member: &ListedMember) -> MemberDTO {
    MemberDTO {
        account: member.account.to_string(),
        role: named(member.role).to_owned(),
        name: member.name.clone(),
        email: member.email.clone(),
        leaving_until: member.leaving_until.map(text::serialize),
        picture: member.picture.clone(),
    }
}

pub(crate) fn to_membership_dto(membership: &Membership) -> MembershipDTO {
    MembershipDTO {
        project: membership.project.to_string(),
        role: named(membership.role).to_owned(),
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
