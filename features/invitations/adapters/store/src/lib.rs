use email::EmailAddress;
use eventsourcing::Codec;
use invitations_core::{InvitationEvent, Outcome};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Deserialize)]
enum StoredInvitationEvent {
    Issued {
        to: String,
    },
    Accepted,
    Withdrawn,
    Snapshotted {
        to: String,
        #[serde(with = "time::serde::rfc3339")]
        issued_at: OffsetDateTime,
        outcome: StoredOutcome,
    },
}

#[derive(Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
enum StoredOutcome {
    Open,
    Accepted,
    Withdrawn,
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
        }
    }
}

impl From<StoredOutcome> for Outcome {
    fn from(outcome: StoredOutcome) -> Self {
        match outcome {
            StoredOutcome::Open => Self::Open,
            StoredOutcome::Accepted => Self::Accepted,
            StoredOutcome::Withdrawn => Self::Withdrawn,
        }
    }
}

impl From<&InvitationEvent> for StoredInvitationEvent {
    fn from(event: &InvitationEvent) -> Self {
        match event {
            InvitationEvent::Issued { to } => Self::Issued { to: to.to_string() },
            InvitationEvent::Accepted => Self::Accepted,
            InvitationEvent::Withdrawn => Self::Withdrawn,
            InvitationEvent::Snapshotted {
                to,
                issued_at,
                outcome,
            } => Self::Snapshotted {
                to: to.to_string(),
                issued_at: *issued_at,
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
            StoredInvitationEvent::Issued { to } => Self::Issued { to: readable(&to)? },
            StoredInvitationEvent::Accepted => Self::Accepted,
            StoredInvitationEvent::Withdrawn => Self::Withdrawn,
            StoredInvitationEvent::Snapshotted {
                to,
                issued_at,
                outcome,
            } => Self::Snapshotted {
                to: readable(&to)?,
                issued_at,
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
        for outcome in [Outcome::Open, Outcome::Accepted, Outcome::Withdrawn] {
            for event in [
                InvitationEvent::Issued { to: ada() },
                InvitationEvent::Accepted,
                InvitationEvent::Withdrawn,
                InvitationEvent::Snapshotted {
                    to: ada(),
                    issued_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_000),
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
            json!({ "Issued": { "to": "ada" } }),
            json!({ "Snapshotted": { "to": "ada", "issued_at": "1970-01-01T00:16:40Z", "outcome": "open" } }),
        ] {
            assert!((codec().event)(refused.clone()).is_none(), "{refused}");
        }
    }
}
