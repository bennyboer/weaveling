use crate::members::listed::{Members, Role};
use crate::working_copy;

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Intent {
    ChangeRole { account: String, to: Role },
    Remove { account: String },
}

impl working_copy::Intent<Members> for Intent {
    fn lay_over(&self, on: &mut Members) {
        match self {
            Self::ChangeRole { account, to } => {
                if let Some(member) = on
                    .people
                    .iter_mut()
                    .find(|member| &member.account == account)
                {
                    member.role = *to;
                }
            }
            Self::Remove { account } => on.people.retain(|member| &member.account != account),
        }
    }

    fn merged_after(&self, earlier: &Self) -> Option<Self> {
        match (self, earlier) {
            (
                Self::ChangeRole { account, .. },
                Self::ChangeRole {
                    account: before, ..
                },
            ) if account == before => Some(self.clone()),
            _ => None,
        }
    }
}

impl Intent {
    pub fn landed_on(&self, on: &Members) -> Members {
        let mut landed = on.clone();
        working_copy::Intent::lay_over(self, &mut landed);
        landed.version = on.version + 1;

        landed
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::members::listed::Member;
    use crate::working_copy::Intent as _;

    fn a_member(account: &str, role: Role) -> Member {
        Member {
            account: account.to_owned(),
            role,
            name: None,
            email: None,
        }
    }

    fn some_members() -> Members {
        Members {
            version: 4,
            people: vec![
                a_member("account_ada", Role::Owner),
                a_member("account_grace", Role::Author),
                a_member("account_hedy", Role::Reader),
            ],
        }
    }

    fn roles(members: &Members) -> Vec<(&str, Role)> {
        members
            .people
            .iter()
            .map(|member| (member.account.as_str(), member.role))
            .collect()
    }

    fn changing(account: &str, to: Role) -> Intent {
        Intent::ChangeRole {
            account: account.to_owned(),
            to,
        }
    }

    #[test]
    fn a_change_of_role_shows_the_member_in_the_new_role_and_the_rest_as_they_were() {
        // given: some members
        let mut members = some_members();

        // when: a change of role is laid over them
        changing("account_grace", Role::Owner).lay_over(&mut members);

        // then: that member has the new role, the others theirs
        assert_eq!(
            roles(&members),
            vec![
                ("account_ada", Role::Owner),
                ("account_grace", Role::Owner),
                ("account_hedy", Role::Reader),
            ]
        );
    }

    #[test]
    fn a_removal_shows_the_member_gone_and_one_of_nobody_held_changes_nothing() {
        // given: some members
        let mut members = some_members();

        // when: a member is removed, and an account that is none of them
        Intent::Remove {
            account: "account_hedy".to_owned(),
        }
        .lay_over(&mut members);
        Intent::Remove {
            account: "account_lin".to_owned(),
        }
        .lay_over(&mut members);

        // then: only that member is gone
        assert_eq!(
            roles(&members),
            vec![
                ("account_ada", Role::Owner),
                ("account_grace", Role::Author)
            ]
        );
    }

    #[test]
    fn role_changes_of_one_member_close_together_go_out_as_the_last() {
        // expect: a later change of the same member's role stands for both
        assert_eq!(
            changing("account_grace", Role::Owner)
                .merged_after(&changing("account_grace", Role::Reader)),
            Some(changing("account_grace", Role::Owner))
        );
    }

    #[test]
    fn changes_of_different_members_or_removals_go_out_each_in_turn() {
        // expect: none of them merges with what came before
        for (later, earlier) in [
            (
                changing("account_grace", Role::Owner),
                changing("account_hedy", Role::Owner),
            ),
            (
                Intent::Remove {
                    account: "account_grace".to_owned(),
                },
                changing("account_grace", Role::Owner),
            ),
            (
                changing("account_grace", Role::Owner),
                Intent::Remove {
                    account: "account_grace".to_owned(),
                },
            ),
        ] {
            assert_eq!(later.merged_after(&earlier), None);
        }
    }

    #[test]
    fn a_removal_that_landed_is_the_members_without_them_one_version_on() {
        // when: a removal lands on some members
        let landed = Intent::Remove {
            account: "account_grace".to_owned(),
        }
        .landed_on(&some_members());

        // then: the members are those without them, one version on
        assert_eq!(
            roles(&landed),
            vec![("account_ada", Role::Owner), ("account_hedy", Role::Reader)]
        );
        assert_eq!(
            landed.version, 5,
            "a removal is one change to the members, and the server tells nothing back"
        );
    }
}
