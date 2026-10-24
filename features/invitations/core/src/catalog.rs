use std::error::Error;

use async_trait::async_trait;
use eventsourcing::Version;
use thiserror::Error;
use time::{Duration, OffsetDateTime};

use crate::id::InvitationId;
use crate::invitation::{DeclineReason, Invitation, Outcome};
use email::EmailAddress;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct InvitationSummary {
    pub id: InvitationId,
    pub version: Version,
    pub to: EmailAddress,
    pub issued_at: OffsetDateTime,
    pub until: OffsetDateTime,
    pub outcome: Outcome,
    pub forgotten_after: OffsetDateTime,
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

    async fn forget_due(&self, now: OffsetDateTime) -> Result<u64, CatalogError>;
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
            issued_at: invitation.issued_at(),
            until: invitation.until(),
            outcome: invitation.outcome(),
            forgotten_after: forgotten_after(invitation),
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

fn forgotten_after(invitation: &Invitation) -> OffsetDateTime {
    match (invitation.outcome(), invitation.settled_at()) {
        (Outcome::Declined(_), Some(settled)) => settled + InvitationSummary::DECLINED_LISTED_FOR,
        (Outcome::Accepted | Outcome::Withdrawn, Some(settled)) => {
            settled + InvitationSummary::SETTLED_KEPT_FOR
        }
        _ => invitation.until() + InvitationSummary::EXPIRED_LISTED_FOR,
    }
}

#[cfg(test)]
mod tests {
    use eventsourcing::{Agent, Aggregate, AggregateId, EventMetadata};
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
                forgotten_after: at(5_000) + Duration::days(30),
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
            forgotten_after: at(5_000),
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
            forgotten_after: at(5_000),
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
            forgotten_after: at(5_000),
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
    fn a_summary_is_forgotten_a_month_after_running_out_or_being_declined_and_a_day_after_settling_otherwise()
     {
        let issued = || {
            Invitation::from_first(
                &InvitationEvent::Issued {
                    to: ada(),
                    digest: InvitationTokenDigest::kept("ab12".to_owned()),
                    until: at(5_000),
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
            .forgotten_after
        };

        assert_eq!(
            InvitationSummary::of(InvitationId::generate(at(1_000)), Version::of(1), &issued())
                .forgotten_after,
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
        for gone in [InvitationEvent::Accepted, InvitationEvent::Withdrawn] {
            assert_eq!(settled(gone), at(2_000) + Duration::days(1));
        }
    }
}
