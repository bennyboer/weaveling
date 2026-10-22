use accounts_core::{AccountEvent, AccountName, EmailAddress};
use eventsourcing::Codec;
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Deserialize)]
enum StoredAccountEvent {
    SignedUp {
        email: String,
        name: String,
    },
    EmailVerified,
    PasswordReset,
    Snapshotted {
        email: String,
        name: String,
        verified: bool,
        #[serde(with = "time::serde::rfc3339")]
        signed_up_at: OffsetDateTime,
        #[serde(default, with = "time::serde::rfc3339::option")]
        sessions_since: Option<OffsetDateTime>,
    },
}

pub fn codec() -> Codec<AccountEvent> {
    Codec {
        body: |event| {
            serde_json::to_value(StoredAccountEvent::from(event))
                .expect("a stored account event is plain data and cannot fail to serialize")
        },
        event: |body| {
            serde_json::from_value::<StoredAccountEvent>(body)
                .ok()
                .and_then(|stored| AccountEvent::try_from(stored).ok())
        },
    }
}

impl From<&AccountEvent> for StoredAccountEvent {
    fn from(event: &AccountEvent) -> Self {
        match event {
            AccountEvent::SignedUp { email, name } => Self::SignedUp {
                email: email.to_string(),
                name: name.to_string(),
            },
            AccountEvent::EmailVerified => Self::EmailVerified,
            AccountEvent::PasswordReset => Self::PasswordReset,
            AccountEvent::Snapshotted {
                email,
                name,
                verified,
                signed_up_at,
                sessions_since,
            } => Self::Snapshotted {
                email: email.to_string(),
                name: name.to_string(),
                verified: *verified,
                signed_up_at: *signed_up_at,
                sessions_since: *sessions_since,
            },
        }
    }
}

#[derive(Debug)]
struct UnreadableAccountEvent;

fn readable(
    email: &str,
    name: &str,
) -> Result<(EmailAddress, AccountName), UnreadableAccountEvent> {
    Ok((
        EmailAddress::new(email).map_err(|_| UnreadableAccountEvent)?,
        AccountName::new(name).map_err(|_| UnreadableAccountEvent)?,
    ))
}

impl TryFrom<StoredAccountEvent> for AccountEvent {
    type Error = UnreadableAccountEvent;

    fn try_from(stored: StoredAccountEvent) -> Result<Self, Self::Error> {
        Ok(match stored {
            StoredAccountEvent::SignedUp { email, name } => {
                let (email, name) = readable(&email, &name)?;
                Self::SignedUp { email, name }
            }
            StoredAccountEvent::EmailVerified => Self::EmailVerified,
            StoredAccountEvent::PasswordReset => Self::PasswordReset,
            StoredAccountEvent::Snapshotted {
                email,
                name,
                verified,
                signed_up_at,
                sessions_since,
            } => {
                let (email, name) = readable(&email, &name)?;
                Self::Snapshotted {
                    email,
                    name,
                    verified,
                    signed_up_at,
                    sessions_since,
                }
            }
        })
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;
    use time::Duration;

    use super::*;

    fn round_trip(event: AccountEvent) -> AccountEvent {
        let codec = codec();

        (codec.event)((codec.body)(&event)).expect("what we just wrote should be readable")
    }

    fn ada() -> (EmailAddress, AccountName) {
        (
            EmailAddress::new("ada@example.com").expect("a plain address"),
            AccountName::new("Ada").expect("a plain name"),
        )
    }

    #[test]
    fn every_event_survives_the_journey_through_the_column() {
        let (email, name) = ada();

        for event in [
            AccountEvent::SignedUp {
                email: email.clone(),
                name: name.clone(),
            },
            AccountEvent::EmailVerified,
            AccountEvent::PasswordReset,
            AccountEvent::Snapshotted {
                email,
                name,
                verified: true,
                signed_up_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_000),
                sessions_since: Some(OffsetDateTime::UNIX_EPOCH + Duration::seconds(2_000)),
            },
        ] {
            assert_eq!(round_trip(event.clone()), event);
        }
    }

    #[test]
    fn a_stored_address_or_name_the_domain_would_refuse_reads_as_nothing() {
        for refused in [
            json!({ "SignedUp": { "email": "ada", "name": "Ada" } }),
            json!({ "SignedUp": { "email": "ada@example.com", "name": "  " } }),
            json!({ "Snapshotted": { "email": "ada", "name": "Ada", "verified": true, "signed_up_at": "1970-01-01T00:16:40Z" } }),
            json!({ "Snapshotted": { "email": "ada@example.com", "name": " ", "verified": true, "signed_up_at": "1970-01-01T00:16:40Z" } }),
        ] {
            assert!((codec().event)(refused.clone()).is_none(), "{refused}");
        }
    }

    #[test]
    fn a_snapshots_moment_is_stored_in_a_shape_a_human_can_read() {
        let (email, name) = ada();
        let written = (codec().body)(&AccountEvent::Snapshotted {
            email,
            name,
            verified: false,
            signed_up_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_000),
            sessions_since: Some(OffsetDateTime::UNIX_EPOCH + Duration::seconds(2_000)),
        });

        assert_eq!(
            written["Snapshotted"]["signed_up_at"],
            json!("1970-01-01T00:16:40Z")
        );
    }
}
