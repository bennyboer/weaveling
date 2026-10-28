use std::error::Error;

use async_trait::async_trait;
use eventsourcing::{AccountId, Version};
use thiserror::Error;
use time::{Duration, OffsetDateTime};

use crate::id::InvitationId;
use crate::invitation::{DeclineReason, Invitation, Outcome};
use crate::scope::{ProjectLink, Scope};
use email::EmailAddress;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvitationSummary {
    pub id: InvitationId,
    pub version: Version,
    pub to: EmailAddress,
    pub scope: Scope,
    pub issued_at: OffsetDateTime,
    pub until: OffsetDateTime,
    pub outcome: Outcome,
    pub retire_after: OffsetDateTime,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Status {
    Open,
    Expired,
    Declined(DeclineReason),
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ListedInvitation {
    pub summary: InvitationSummary,
    pub status: Status,
}

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Person {
    pub account: AccountId,
    pub name: String,
    pub email: EmailAddress,
}

#[derive(Debug, Error)]
pub enum CatalogError {
    #[error("the list of invitations could not be reached")]
    Backend(#[source] Box<dyn Error + Send + Sync>),
}

#[async_trait]
pub trait InvitationCatalog: Send + Sync {
    async fn remember(&self, summary: &InvitationSummary) -> Result<(), CatalogError>;

    async fn listed_after(
        &self,
        after: Option<&InvitationId>,
        at_most: usize,
    ) -> Result<Vec<InvitationSummary>, CatalogError>;

    async fn in_project_after(
        &self,
        project: &ProjectLink,
        after: Option<&InvitationId>,
        at_most: usize,
    ) -> Result<Vec<InvitationSummary>, CatalogError>;

    async fn due_for_retiring(
        &self,
        now: OffsetDateTime,
        at_most: usize,
    ) -> Result<Vec<InvitationId>, CatalogError>;

    async fn forget(&self, invitation: &InvitationId) -> Result<(), CatalogError>;

    async fn remember_person(&self, person: &Person) -> Result<(), CatalogError>;

    async fn people(&self, accounts: &[AccountId]) -> Result<Vec<Person>, CatalogError>;
}

impl InvitationSummary {
    pub const EXPIRED_LISTED_FOR: Duration = Duration::days(30);
    pub const DECLINED_LISTED_FOR: Duration = Duration::days(30);
    pub const SETTLED_KEPT_FOR: Duration = Duration::days(1);

    pub fn of(id: InvitationId, version: Version, invitation: &Invitation) -> Self {
        Self {
            id,
            version,
            to: invitation.to().clone(),
            scope: invitation.scope().clone(),
            issued_at: invitation.issued_at(),
            until: invitation.until(),
            outcome: invitation.outcome(),
            retire_after: retire_after(invitation),
        }
    }

    pub fn status(&self, now: OffsetDateTime) -> Option<Status> {
        match self.outcome {
            Outcome::Declined(reason) => Some(Status::Declined(reason)),
            Outcome::Open if now > self.until => Some(Status::Expired),
            Outcome::Open => Some(Status::Open),
            Outcome::Accepted | Outcome::Withdrawn => None,
        }
    }

    pub fn stays_listed(&self) -> bool {
        matches!(self.outcome, Outcome::Open | Outcome::Declined(_))
    }
}

fn retire_after(invitation: &Invitation) -> OffsetDateTime {
    match (invitation.outcome(), invitation.settled_at()) {
        (Outcome::Declined(_), Some(settled)) => settled + InvitationSummary::DECLINED_LISTED_FOR,
        (Outcome::Accepted | Outcome::Withdrawn, Some(settled)) => {
            (settled + InvitationSummary::SETTLED_KEPT_FOR).max(invitation.until())
        }
        _ => invitation.until() + InvitationSummary::EXPIRED_LISTED_FOR,
    }
}

#[cfg(test)]
mod tests {
    use eventsourcing::{Agent, Aggregate, AggregateContext, AggregateId, EventMetadata};
    use time::Duration;

    use super::*;
    use crate::invitation::{InvitationEvent, KIND};
    use crate::token::InvitationTokenDigest;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn stamped(version: u64, seconds: i64) -> EventMetadata {
        EventMetadata {
            aggregate: AggregateId::from("invitation_1"),
            kind: KIND,
            version: Version::of(version),
            agent: Agent::System,
            occurred_at: at(seconds),
            is_snapshot: false,
            context: AggregateContext::default(),
        }
    }

    fn ada() -> EmailAddress {
        EmailAddress::new("ada@example.com").expect("a plain address")
    }

    #[test]
    fn a_summary_tells_to_whom_since_when_until_when_and_how_it_stands() {
        let invitation = Invitation::from_first(
            &InvitationEvent::Issued {
                to: ada(),
                digest: InvitationTokenDigest::kept("ab12".to_owned()),
                until: at(5_000),
                scope: Scope::Instance,
                by_an_operator: true,
            },
            &stamped(1, 1_000),
        )
        .expect("issuing gives birth");
        let id = InvitationId::generate(at(1_000));

        assert_eq!(
            InvitationSummary::of(id, Version::of(3), &invitation),
            InvitationSummary {
                id,
                version: Version::of(3),
                to: ada(),
                issued_at: at(1_000),
                until: at(5_000),
                outcome: Outcome::Open,
                retire_after: at(5_000) + Duration::days(30),
                scope: Scope::Instance,
            }
        );

        let mut declined = invitation;
        declined.apply(
            &InvitationEvent::Declined {
                reason: DeclineReason::AddressHasAccount,
            },
            &stamped(2, 1_100),
        );
        assert_eq!(
            InvitationSummary::of(id, Version::of(4), &declined).outcome,
            Outcome::Declined(DeclineReason::AddressHasAccount)
        );
    }

    #[test]
    fn only_an_invitation_open_or_declined_stays_on_the_list() {
        let summary = |outcome| InvitationSummary {
            id: InvitationId::generate(at(1_000)),
            version: Version::of(1),
            to: ada(),
            issued_at: at(1_000),
            until: at(5_000),
            outcome,
            retire_after: at(5_000),
            scope: Scope::Instance,
        };

        assert!(summary(Outcome::Open).stays_listed());
        assert!(summary(Outcome::Declined(DeclineReason::AddressHasAccount)).stays_listed());
        assert!(!summary(Outcome::Accepted).stays_listed());
        assert!(!summary(Outcome::Withdrawn).stays_listed());
    }

    #[test]
    fn an_open_invitation_is_listed_open_to_its_last_moment_and_expired_after() {
        let summary = InvitationSummary {
            id: InvitationId::generate(at(1_000)),
            version: Version::of(1),
            to: ada(),
            issued_at: at(1_000),
            until: at(5_000),
            outcome: Outcome::Open,
            retire_after: at(5_000),
            scope: Scope::Instance,
        };

        assert_eq!(summary.status(at(5_000)), Some(Status::Open));
        assert_eq!(summary.status(at(5_001)), Some(Status::Expired));
    }

    #[test]
    fn a_declined_invitation_is_listed_declined_whenever_and_the_settled_not_at_all() {
        let summary = |outcome| InvitationSummary {
            id: InvitationId::generate(at(1_000)),
            version: Version::of(1),
            to: ada(),
            issued_at: at(1_000),
            until: at(5_000),
            outcome,
            retire_after: at(5_000),
            scope: Scope::Instance,
        };
        let declined = Outcome::Declined(DeclineReason::AddressHasAccount);

        for now in [at(2_000), at(9_000)] {
            assert_eq!(
                summary(declined).status(now),
                Some(Status::Declined(DeclineReason::AddressHasAccount)),
                "why it was declined tells more than that it ran out"
            );
            assert_eq!(summary(Outcome::Accepted).status(now), None);
            assert_eq!(summary(Outcome::Withdrawn).status(now), None);
        }
    }

    #[test]
    fn an_invitation_retires_a_month_after_running_out_or_being_declined_and_a_day_after_settling_but_never_before_running_out()
     {
        let issued = || {
            Invitation::from_first(
                &InvitationEvent::Issued {
                    to: ada(),
                    digest: InvitationTokenDigest::kept("ab12".to_owned()),
                    until: at(5_000),
                    scope: Scope::Instance,
                    by_an_operator: true,
                },
                &stamped(1, 1_000),
            )
            .expect("issuing gives birth")
        };
        let settled = |event: InvitationEvent| {
            let mut invitation = issued();
            invitation.apply(&event, &stamped(2, 2_000));
            InvitationSummary::of(
                InvitationId::generate(at(1_000)),
                Version::of(2),
                &invitation,
            )
            .retire_after
        };

        assert_eq!(
            InvitationSummary::of(InvitationId::generate(at(1_000)), Version::of(1), &issued())
                .retire_after,
            at(5_000) + Duration::days(30),
            "an operator sees for a month that an invitation ran out"
        );
        assert_eq!(
            settled(InvitationEvent::Declined {
                reason: DeclineReason::AddressHasAccount
            }),
            at(2_000) + Duration::days(30),
            "and for a month why one was declined"
        );
        for gone in [
            InvitationEvent::Accepted {
                scope: Scope::Instance,
            },
            InvitationEvent::Withdrawn,
        ] {
            let mut holding_long = Invitation::from_first(
                &InvitationEvent::Issued {
                    to: ada(),
                    digest: InvitationTokenDigest::kept("ab12".to_owned()),
                    until: at(500_000),
                    scope: Scope::Instance,
                    by_an_operator: true,
                },
                &stamped(1, 1_000),
            )
            .expect("issuing gives birth");
            holding_long.apply(&gone, &stamped(2, 2_000));
            assert_eq!(
                InvitationSummary::of(
                    InvitationId::generate(at(1_000)),
                    Version::of(2),
                    &holding_long
                )
                .retire_after,
                at(500_000),
                "never before it runs out, or a late issue could open a withdrawn link again"
            );
            assert_eq!(settled(gone), at(2_000) + Duration::days(1));
        }
        let mut long_after = issued();
        long_after.apply(&InvitationEvent::Withdrawn, &stamped(2, 9_000));
        assert_eq!(
            InvitationSummary::of(
                InvitationId::generate(at(1_000)),
                Version::of(2),
                &long_after
            )
            .retire_after,
            at(9_000) + Duration::days(1),
            "withdrawn after it ran out, a day after"
        )
    }
}
