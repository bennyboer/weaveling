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

    fn grace(status: &str, declined_for: Option<&str>) -> InvitationSummaryDTO {
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
        let page = InvitationPage::of(InvitationPageDTO {
            invitations: vec![grace("open", None)],
            next: Some("invitation_1".to_owned()),
        })
        .expect("a page the server sends is readable");

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
        assert_eq!(the_status(grace("expired", None)), Ok(Status::Expired));
        assert_eq!(
            the_status(grace("declined", Some("address-has-account"))),
            Ok(Status::Declined(DeclineReason::AddressHasAccount))
        );
        assert_eq!(
            the_status(grace("declined", Some("turned-down"))),
            Ok(Status::Declined(DeclineReason::TurnedDown))
        );
    }

    #[test]
    fn a_scope_sent_is_read_back_as_it_was() {
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
        for (named, told) in [
            ("owner", Role::Owner),
            ("author", Role::Author),
            ("reader", Role::Reader),
        ] {
            let mut summary = grace("open", None);
            summary.scope = ScopeDTO::Project {
                project: "project_1".to_owned(),
                role: named.to_owned(),
            };

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
        let mut broken = grace("open", None);
        broken.until = "next week".to_owned();

        for unreadable in [
            grace("pending", None),
            grace("declined", None),
            grace("declined", Some("had a bad day")),
            grace("open", Some("address-has-account")),
            grace("expired", Some("turned-down")),
            broken,
            InvitationSummaryDTO {
                scope: ScopeDTO::Project {
                    project: "project_1".to_owned(),
                    role: "editor".to_owned(),
                },
                ..grace("open", None)
            },
        ] {
            assert_eq!(the_status(unreadable), Err(ApiError::Unexpected));
        }
    }
}
