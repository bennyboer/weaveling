use email::EmailAddress;
use eventsourcing::Codec;
use invitations_core::{DeclineReason, InvitationEvent, InvitationTokenDigest, Outcome};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Deserialize)]
enum StoredInvitationEvent {
    Issued {
        to: String,
        digest: String,
        #[serde(with = "time::serde::rfc3339")]
        until: OffsetDateTime,
    },
    Accepted,
    Withdrawn,
    Declined {
        reason: StoredDeclineReason,
    },
    Snapshotted {
        to: String,
        #[serde(with = "time::serde::rfc3339")]
        issued_at: OffsetDateTime,
        #[serde(with = "time::serde::rfc3339")]
        until: OffsetDateTime,
        outcome: StoredOutcome,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum StoredOutcome {
    Open,
    Accepted,
    Withdrawn,
    Declined(StoredDeclineReason),
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
enum StoredDeclineReason {
    AddressHasAccount,
}

impl From<DeclineReason> for StoredDeclineReason {
    fn from(reason: DeclineReason) -> Self {
        match reason {
            DeclineReason::AddressHasAccount => Self::AddressHasAccount,
        }
    }
}

impl From<StoredDeclineReason> for DeclineReason {
    fn from(reason: StoredDeclineReason) -> Self {
        match reason {
            StoredDeclineReason::AddressHasAccount => Self::AddressHasAccount,
        }
    }
}

pub fn codec() -> Codec<InvitationEvent> {
    Codec {
        body: |event| {
            serde_json::to_value(StoredInvitationEvent::from(event))
                .expect("a stored invitation event is plain data and cannot fail to serialize")
        },
        event: |body| {
            serde_json::from_value::<StoredInvitationEvent>(body)
                .ok()
                .and_then(|stored| InvitationEvent::try_from(stored).ok())
        },
    }
}

impl From<Outcome> for StoredOutcome {
    fn from(outcome: Outcome) -> Self {
        match outcome {
            Outcome::Open => Self::Open,
            Outcome::Accepted => Self::Accepted,
            Outcome::Withdrawn => Self::Withdrawn,
            Outcome::Declined(reason) => Self::Declined(reason.into()),
        }
    }
}

impl From<StoredOutcome> for Outcome {
    fn from(outcome: StoredOutcome) -> Self {
        match outcome {
            StoredOutcome::Open => Self::Open,
            StoredOutcome::Accepted => Self::Accepted,
            StoredOutcome::Withdrawn => Self::Withdrawn,
            StoredOutcome::Declined(reason) => Self::Declined(reason.into()),
        }
    }
}

impl From<&InvitationEvent> for StoredInvitationEvent {
    fn from(event: &InvitationEvent) -> Self {
        match event {
            InvitationEvent::Issued { to, digest, until } => Self::Issued {
                to: to.to_string(),
                digest: digest.as_str().to_owned(),
                until: *until,
            },
            InvitationEvent::Accepted => Self::Accepted,
            InvitationEvent::Withdrawn => Self::Withdrawn,
            InvitationEvent::Declined { reason } => Self::Declined {
                reason: (*reason).into(),
            },
            InvitationEvent::Snapshotted {
                to,
                issued_at,
                until,
                outcome,
            } => Self::Snapshotted {
                to: to.to_string(),
                issued_at: *issued_at,
                until: *until,
                outcome: (*outcome).into(),
            },
        }
    }
}

#[derive(Debug)]
struct UnreadableInvitationEvent;

fn readable(to: &str) -> Result<EmailAddress, UnreadableInvitationEvent> {
    EmailAddress::new(to).map_err(|_| UnreadableInvitationEvent)
}

impl TryFrom<StoredInvitationEvent> for InvitationEvent {
    type Error = UnreadableInvitationEvent;

    fn try_from(stored: StoredInvitationEvent) -> Result<Self, Self::Error> {
        Ok(match stored {
            StoredInvitationEvent::Issued { to, digest, until } => Self::Issued {
                to: readable(&to)?,
                digest: InvitationTokenDigest::kept(digest),
                until,
            },
            StoredInvitationEvent::Accepted => Self::Accepted,
            StoredInvitationEvent::Withdrawn => Self::Withdrawn,
            StoredInvitationEvent::Declined { reason } => Self::Declined {
                reason: reason.into(),
            },
            StoredInvitationEvent::Snapshotted {
                to,
                issued_at,
                until,
                outcome,
            } => Self::Snapshotted {
                to: readable(&to)?,
                issued_at,
                until,
                outcome: outcome.into(),
            },
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use time::Duration;

    use super::*;

    fn ada() -> EmailAddress {
        EmailAddress::new("ada@example.com").expect("a plain address")
    }

    fn round_trip(event: InvitationEvent) -> InvitationEvent {
        let codec = codec();

        (codec.event)((codec.body)(&event)).expect("what we just wrote should be readable")
    }

    #[test]
    fn every_event_survives_the_journey_through_the_column() {
        for outcome in [
            Outcome::Open,
            Outcome::Accepted,
            Outcome::Withdrawn,
            Outcome::Declined(DeclineReason::AddressHasAccount),
        ] {
            for event in [
                InvitationEvent::Issued {
                    to: ada(),
                    digest: InvitationTokenDigest::kept("ab12".to_owned()),
                    until: OffsetDateTime::UNIX_EPOCH + Duration::seconds(2_000),
                },
                InvitationEvent::Accepted,
                InvitationEvent::Withdrawn,
                InvitationEvent::Declined {
                    reason: DeclineReason::AddressHasAccount,
                },
                InvitationEvent::Snapshotted {
                    to: ada(),
                    issued_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_000),
                    until: OffsetDateTime::UNIX_EPOCH + Duration::seconds(2_000),
                    outcome,
                },
            ] {
                assert_eq!(round_trip(event.clone()), event);
            }
        }
    }

    #[test]
    fn a_stored_address_the_domain_would_refuse_reads_as_nothing() {
        for refused in [
            json!({ "Issued": { "to": "ada", "digest": "ab12", "until": "1970-01-01T00:33:20Z" } }),
            json!({ "Snapshotted": { "to": "ada", "issued_at": "1970-01-01T00:16:40Z", "until": "1970-01-01T00:33:20Z", "outcome": "open" } }),
        ] {
            assert!((codec().event)(refused.clone()).is_none(), "{refused}");
        }
    }
}
