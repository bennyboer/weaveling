use members_contract::{MemberDTO, role};

use crate::http::ApiError;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Role {
    Owner,
    Author,
    Reader,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Member {
    pub account: String,
    pub role: Role,
    pub name: Option<String>,
    pub email: Option<String>,
}

impl Role {
    pub const ALL: [Role; 3] = [Role::Owner, Role::Author, Role::Reader];

    pub fn named(self) -> &'static str {
        match self {
            Self::Owner => role::OWNER,
            Self::Author => role::AUTHOR,
            Self::Reader => role::READER,
        }
    }

    pub fn of(named: &str) -> Option<Role> {
        role_of(named).ok()
    }

    pub fn message(self) -> &'static str {
        match self {
            Self::Owner => "members-role-owner",
            Self::Author => "members-role-author",
            Self::Reader => "members-role-reader",
        }
    }
}

fn role_of(told: &str) -> Result<Role, ApiError> {
    match told {
        role::OWNER => Ok(Role::Owner),
        role::AUTHOR => Ok(Role::Author),
        role::READER => Ok(Role::Reader),
        _ => Err(ApiError::Unexpected),
    }
}

pub fn listed(member: MemberDTO) -> Result<Member, ApiError> {
    Ok(Member {
        role: role_of(&member.role)?,
        account: member.account,
        name: member.name,
        email: member.email,
    })
}

#[cfg(test)]
mod tests {
    use super::*;

    fn told(role: &str) -> MemberDTO {
        MemberDTO {
            account: "account_1".to_owned(),
            role: role.to_owned(),
            name: Some("Ada Lovelace".to_owned()),
            email: Some("ada@example.com".to_owned()),
        }
    }

    #[test]
    fn each_role_told_is_read_as_that_role_with_whatever_else_was_told() {
        for (named, read) in [
            (role::OWNER, Role::Owner),
            (role::AUTHOR, Role::Author),
            (role::READER, Role::Reader),
        ] {
            assert_eq!(
                listed(told(named)),
                Ok(Member {
                    account: "account_1".to_owned(),
                    role: read,
                    name: Some("Ada Lovelace".to_owned()),
                    email: Some("ada@example.com".to_owned()),
                })
            );
        }
    }

    #[test]
    fn every_role_goes_by_the_name_the_contract_gives_it() {
        for chosen in Role::ALL {
            assert_eq!(Role::of(chosen.named()), Some(chosen));
        }
        assert_eq!(
            Role::ALL.map(Role::named),
            [role::OWNER, role::AUTHOR, role::READER]
        );
        assert_eq!(Role::of("editor"), None);
    }

    #[test]
    fn a_role_nobody_declared_is_not_guessed_at() {
        assert_eq!(listed(told("editor")), Err(ApiError::Unexpected));
    }
}
