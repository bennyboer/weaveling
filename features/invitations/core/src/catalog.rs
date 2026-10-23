use std::error::Error;

use async_trait::async_trait;
use eventsourcing::Version;
use thiserror::Error;
use time::OffsetDateTime;

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
}

impl InvitationSummary {
    pub fn of(id: InvitationId, version: Version, invitation: &Invitation) -> Self {
        Self {
            id,
            version,
            to: invitation.to().clone(),
            issued_at: invitation.issued_at(),
            until: invitation.until(),
            outcome: invitation.outcome(),
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
}
