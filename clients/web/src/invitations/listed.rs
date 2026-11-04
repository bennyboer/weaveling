use invitations_contract::{
    InvitationPageDTO, InvitationSummaryDTO, ScopeDTO, decline_reason, status,
};
use time::OffsetDateTime;
use time::format_description::well_known::Rfc3339;

use crate::http::ApiError;
use crate::members::listed::Role;

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Open,
    Expired,
    Declined(DeclineReason),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DeclineReason {
    AddressHasAccount,
    TurnedDown,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Scope {
    Instance,
    Project { project: String, role: Role },
}

impl Scope {
    pub fn told(&self) -> ScopeDTO {
        match self {
            Self::Instance => ScopeDTO::Instance,
            Self::Project { project, role } => ScopeDTO::Project {
                project: project.clone(),
                role: role.named().to_owned(),
            },
        }
    }

    fn read(told: ScopeDTO) -> Result<Self, ApiError> {
        match told {
            ScopeDTO::Instance => Ok(Self::Instance),
            ScopeDTO::Project { project, role } => Ok(Self::Project {
                role: Role::of(&role).ok_or(ApiError::Unexpected)?,
                project,
            }),
        }
    }
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedInvitation {
    pub id: String,
    pub email: String,
    pub issued_at: OffsetDateTime,
    pub until: OffsetDateTime,
    pub status: Status,
    pub scope: Scope,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvitationPage {
    pub invitations: Vec<ListedInvitation>,
    pub next: Option<String>,
}

fn moment(written: &str) -> Result<OffsetDateTime, ApiError> {
    OffsetDateTime::parse(written, &Rfc3339).map_err(|_| ApiError::Unexpected)
}

fn status_of(told: &str, declined_for: Option<&str>) -> Result<Status, ApiError> {
    match (told, declined_for) {
        (status::OPEN, None) => Ok(Status::Open),
        (status::EXPIRED, None) => Ok(Status::Expired),
        (status::DECLINED, Some(decline_reason::ADDRESS_HAS_ACCOUNT)) => {
            Ok(Status::Declined(DeclineReason::AddressHasAccount))
        }
        (status::DECLINED, Some(decline_reason::TURNED_DOWN)) => {
            Ok(Status::Declined(DeclineReason::TurnedDown))
        }
        _ => Err(ApiError::Unexpected),
    }
}

pub fn listed(summary: InvitationSummaryDTO) -> Result<ListedInvitation, ApiError> {
    Ok(ListedInvitation {
        scope: Scope::read(summary.scope)?,
        status: status_of(&summary.status, summary.declined_for.as_deref())?,
        issued_at: moment(&summary.issued_at)?,
        until: moment(&summary.until)?,
        id: summary.id,
        email: summary.email,
    })
}

impl InvitationPage {
    pub fn of(page: InvitationPageDTO) -> Result<Self, ApiError> {
        Ok(Self {
            invitations: page
                .invitations
                .into_iter()
                .map(listed)
                .collect::<Result<_, _>>()?,
            next: page.next,
        })
    }
}

#[cfg(test)]
mod tests {
    use time::macros::datetime;

    use super::*;

    fn an_invitation_to_grace(status: &str, declined_for: Option<&str>) -> InvitationSummaryDTO {
        InvitationSummaryDTO {
            id: "invitation_1".to_owned(),
            email: "grace@example.com".to_owned(),
            issued_at: "2026-10-23T08:00:00Z".to_owned(),
            until: "2026-10-30T08:00:00Z".to_owned(),
            status: status.to_owned(),
            declined_for: declined_for.map(str::to_owned),
            scope: ScopeDTO::Instance,
        }
    }

    fn the_status(summary: InvitationSummaryDTO) -> Result<Status, ApiError> {
        InvitationPage::of(InvitationPageDTO {
            invitations: vec![summary],
            next: None,
        })
        .map(|page| page.invitations[0].status)
    }

    #[test]
    fn an_invitation_is_listed_as_the_server_told_it() {
        // when: a page of one open invitation is read
        let page = InvitationPage::of(InvitationPageDTO {
            invitations: vec![an_invitation_to_grace("open", None)],
            next: Some("invitation_1".to_owned()),
        })
        .expect("a page the server sends is readable");

        // then: it lists the invitation and the next page as told
        assert_eq!(
            page,
            InvitationPage {
                invitations: vec![ListedInvitation {
                    id: "invitation_1".to_owned(),
                    email: "grace@example.com".to_owned(),
                    issued_at: datetime!(2026-10-23 08:00 UTC),
                    until: datetime!(2026-10-30 08:00 UTC),
                    status: Status::Open,
                    scope: Scope::Instance,
                }],
                next: Some("invitation_1".to_owned()),
            }
        );
    }

    #[test]
    fn every_status_the_server_tells_is_read_with_why_for_a_decline() {
        // expect: an expired invitation and either kind of declined one read as such
        assert_eq!(
            the_status(an_invitation_to_grace("expired", None)),
            Ok(Status::Expired)
        );
        assert_eq!(
            the_status(an_invitation_to_grace(
                "declined",
                Some("address-has-account")
            )),
            Ok(Status::Declined(DeclineReason::AddressHasAccount))
        );
        assert_eq!(
            the_status(an_invitation_to_grace("declined", Some("turned-down"))),
            Ok(Status::Declined(DeclineReason::TurnedDown))
        );
    }

    #[test]
    fn a_scope_sent_is_read_back_as_it_was() {
        // expect: every scope told and read back is the same
        for scope in [
            Scope::Instance,
            Scope::Project {
                project: "project_1".to_owned(),
                role: Role::Owner,
            },
            Scope::Project {
                project: "project_1".to_owned(),
                role: Role::Reader,
            },
        ] {
            assert_eq!(Scope::read(scope.told()), Ok(scope));
        }
        // and: a project's scope is told with its role by name
        assert_eq!(
            Scope::Project {
                project: "project_1".to_owned(),
                role: Role::Author,
            }
            .told(),
            ScopeDTO::Project {
                project: "project_1".to_owned(),
                role: "author".to_owned(),
            }
        );
    }

    #[test]
    fn a_project_s_invitation_tells_the_project_and_the_role_it_invites_as() {
        // given: an invitation to a project as each role
        for (named, told) in [
            ("owner", Role::Owner),
            ("author", Role::Author),
            ("reader", Role::Reader),
        ] {
            let mut summary = an_invitation_to_grace("open", None);
            summary.scope = ScopeDTO::Project {
                project: "project_1".to_owned(),
                role: named.to_owned(),
            };

            // then: it is listed with that project and role
            assert_eq!(
                listed(summary).map(|invitation| invitation.scope),
                Ok(Scope::Project {
                    project: "project_1".to_owned(),
                    role: told,
                })
            );
        }
    }

    #[test]
    fn a_status_or_a_moment_nobody_declared_is_unexpected() {
        // given: an invitation whose last moment is no moment
        let mut broken = an_invitation_to_grace("open", None);
        broken.until = "next week".to_owned();

        // expect: it and any undeclared status, reason or role are unexpected
        for unreadable in [
            an_invitation_to_grace("pending", None),
            an_invitation_to_grace("declined", None),
            an_invitation_to_grace("declined", Some("had a bad day")),
            an_invitation_to_grace("open", Some("address-has-account")),
            an_invitation_to_grace("expired", Some("turned-down")),
            broken,
            InvitationSummaryDTO {
                scope: ScopeDTO::Project {
                    project: "project_1".to_owned(),
                    role: "editor".to_owned(),
                },
                ..an_invitation_to_grace("open", None)
            },
        ] {
            assert_eq!(the_status(unreadable), Err(ApiError::Unexpected));
        }
    }
}
