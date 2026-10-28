use email::EmailAddress;
use eventsourcing::{AccountId, Codec};
use invitations_core::{
    DeclineReason, InvitationEvent, InvitationTokenDigest, Outcome, ProjectLink, Role, Scope,
};
use serde::{Deserialize, Serialize};
use time::OffsetDateTime;

#[derive(Serialize, Deserialize)]
enum StoredInvitationEvent {
    Issued {
        to: String,
        digest: String,
        #[serde(with = "time::serde::rfc3339")]
        until: OffsetDateTime,
        #[serde(default)]
        scope: StoredScope,
        #[serde(default)]
        by_an_operator: bool,
    },
    Accepted {
        #[serde(default)]
        scope: StoredScope,
    },
    Withdrawn,
    Declined {
        reason: StoredDeclineReason,
    },
    Retired,
    Snapshotted {
        to: String,
        #[serde(default)]
        scope: StoredScope,
        #[serde(default)]
        by_an_operator: bool,
        issued_by: Option<String>,
        #[serde(with = "time::serde::rfc3339")]
        issued_at: OffsetDateTime,
        #[serde(with = "time::serde::rfc3339")]
        until: OffsetDateTime,
        outcome: StoredOutcome,
        #[serde(default, with = "time::serde::rfc3339::option")]
        settled_at: Option<OffsetDateTime>,
        #[serde(default)]
        retired: bool,
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
enum StoredRole {
    Owner,
    Author,
    Reader,
}

impl From<Role> for StoredRole {
    fn from(role: Role) -> Self {
        match role {
            Role::Owner => Self::Owner,
            Role::Author => Self::Author,
            Role::Reader => Self::Reader,
        }
    }
}

impl From<StoredRole> for Role {
    fn from(role: StoredRole) -> Self {
        match role {
            StoredRole::Owner => Self::Owner,
            StoredRole::Author => Self::Author,
            StoredRole::Reader => Self::Reader,
        }
    }
}

#[derive(Serialize, Deserialize, Default)]
#[serde(tag = "kind", rename_all = "kebab-case")]
enum StoredScope {
    #[default]
    Instance,
    Project {
        project: String,
        role: StoredRole,
    },
}

impl From<&Scope> for StoredScope {
    fn from(scope: &Scope) -> Self {
        match scope {
            Scope::Instance => Self::Instance,
            Scope::Project { project, role } => Self::Project {
                project: project.to_string(),
                role: (*role).into(),
            },
        }
    }
}

impl From<StoredScope> for Scope {
    fn from(scope: StoredScope) -> Self {
        match scope {
            StoredScope::Instance => Self::Instance,
            StoredScope::Project { project, role } => Self::Project {
                project: ProjectLink::from(project),
                role: role.into(),
            },
        }
    }
}

#[derive(Serialize, Deserialize, Clone, Copy)]
#[serde(rename_all = "kebab-case")]
enum StoredDeclineReason {
    AddressHasAccount,
    TurnedDown,
}

impl From<DeclineReason> for StoredDeclineReason {
    fn from(reason: DeclineReason) -> Self {
        match reason {
            DeclineReason::AddressHasAccount => Self::AddressHasAccount,
            DeclineReason::TurnedDown => Self::TurnedDown,
        }
    }
}

impl From<StoredDeclineReason> for DeclineReason {
    fn from(reason: StoredDeclineReason) -> Self {
        match reason {
            StoredDeclineReason::AddressHasAccount => Self::AddressHasAccount,
            StoredDeclineReason::TurnedDown => Self::TurnedDown,
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
            InvitationEvent::Issued {
                to,
                digest,
                until,
                scope,
                by_an_operator,
            } => Self::Issued {
                to: to.to_string(),
                digest: digest.as_str().to_owned(),
                until: *until,
                scope: scope.into(),
                by_an_operator: *by_an_operator,
            },
            InvitationEvent::Accepted { scope } => Self::Accepted {
                scope: scope.into(),
            },
            InvitationEvent::Withdrawn => Self::Withdrawn,
            InvitationEvent::Declined { reason } => Self::Declined {
                reason: (*reason).into(),
            },
            InvitationEvent::Retired => Self::Retired,
            InvitationEvent::Snapshotted {
                to,
                scope,
                by_an_operator,
                issued_by,
                issued_at,
                until,
                outcome,
                settled_at,
                retired,
            } => Self::Snapshotted {
                to: to.to_string(),
                scope: scope.into(),
                by_an_operator: *by_an_operator,
                issued_by: issued_by.as_ref().map(ToString::to_string),
                issued_at: *issued_at,
                until: *until,
                outcome: (*outcome).into(),
                settled_at: *settled_at,
                retired: *retired,
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
            StoredInvitationEvent::Issued {
                to,
                digest,
                until,
                scope,
                by_an_operator,
            } => Self::Issued {
                to: readable(&to)?,
                digest: InvitationTokenDigest::kept(digest),
                until,
                scope: scope.into(),
                by_an_operator,
            },
            StoredInvitationEvent::Accepted { scope } => Self::Accepted {
                scope: scope.into(),
            },
            StoredInvitationEvent::Withdrawn => Self::Withdrawn,
            StoredInvitationEvent::Declined { reason } => Self::Declined {
                reason: reason.into(),
            },
            StoredInvitationEvent::Retired => Self::Retired,
            StoredInvitationEvent::Snapshotted {
                to,
                scope,
                by_an_operator,
                issued_by,
                issued_at,
                until,
                outcome,
                settled_at,
                retired,
            } => Self::Snapshotted {
                to: readable(&to)?,
                scope: scope.into(),
                by_an_operator,
                issued_by: issued_by.map(AccountId::from),
                issued_at,
                until,
                outcome: outcome.into(),
                settled_at,
                retired,
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

    fn a_project_s(role: Role) -> Scope {
        Scope::Project {
            project: ProjectLink::from("project_1"),
            role,
        }
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
            Outcome::Declined(DeclineReason::TurnedDown),
        ] {
            for event in [
                InvitationEvent::Issued {
                    to: ada(),
                    digest: InvitationTokenDigest::kept("ab12".to_owned()),
                    until: OffsetDateTime::UNIX_EPOCH + Duration::seconds(2_000),
                    scope: Scope::Instance,
                    by_an_operator: true,
                },
                InvitationEvent::Issued {
                    to: ada(),
                    digest: InvitationTokenDigest::kept("ab12".to_owned()),
                    until: OffsetDateTime::UNIX_EPOCH + Duration::seconds(2_000),
                    scope: a_project_s(Role::Reader),
                    by_an_operator: false,
                },
                InvitationEvent::Accepted {
                    scope: Scope::Instance,
                },
                InvitationEvent::Accepted {
                    scope: a_project_s(Role::Owner),
                },
                InvitationEvent::Withdrawn,
                InvitationEvent::Declined {
                    reason: DeclineReason::AddressHasAccount,
                },
                InvitationEvent::Declined {
                    reason: DeclineReason::TurnedDown,
                },
                InvitationEvent::Snapshotted {
                    to: ada(),
                    scope: a_project_s(Role::Author),
                    by_an_operator: true,
                    issued_by: Some(AccountId::from("account_grace")),
                    issued_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_000),
                    until: OffsetDateTime::UNIX_EPOCH + Duration::seconds(2_000),
                    outcome,
                    settled_at: Some(OffsetDateTime::UNIX_EPOCH + Duration::seconds(3_000)),
                    retired: true,
                },
                InvitationEvent::Snapshotted {
                    to: ada(),
                    scope: Scope::Instance,
                    by_an_operator: false,
                    issued_by: None,
                    issued_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_000),
                    until: OffsetDateTime::UNIX_EPOCH + Duration::seconds(2_000),
                    outcome,
                    settled_at: None,
                    retired: false,
                },
                InvitationEvent::Retired,
            ] {
                assert_eq!(round_trip(event.clone()), event);
            }
        }
    }

    #[test]
    fn every_role_survives_and_an_invitation_stored_before_scopes_is_the_instance_s() {
        for role in [Role::Owner, Role::Author, Role::Reader] {
            let issued = InvitationEvent::Issued {
                to: ada(),
                digest: InvitationTokenDigest::kept("ab12".to_owned()),
                until: OffsetDateTime::UNIX_EPOCH,
                scope: a_project_s(role),
                by_an_operator: true,
            };
            assert_eq!(round_trip(issued.clone()), issued);
        }

        assert_eq!(
            (codec().event)(
                json!({ "Issued": { "to": "ada@example.com", "digest": "ab12", "until": "1970-01-01T00:33:20Z" } })
            ),
            Some(InvitationEvent::Issued {
                to: ada(),
                digest: InvitationTokenDigest::kept("ab12".to_owned()),
                until: OffsetDateTime::UNIX_EPOCH + Duration::seconds(2_000),
                scope: Scope::Instance,
                by_an_operator: false,
            })
        );
    }

    #[test]
    fn a_project_without_its_role_or_a_scope_of_no_known_kind_reads_as_nothing() {
        for refused in [
            json!({ "Issued": { "to": "ada@example.com", "digest": "ab12", "until": "1970-01-01T00:33:20Z", "scope": { "kind": "project", "project": "project_1" } } }),
            json!({ "Issued": { "to": "ada@example.com", "digest": "ab12", "until": "1970-01-01T00:33:20Z", "scope": { "kind": "shelf" } } }),
        ] {
            assert!((codec().event)(refused.clone()).is_none(), "{refused}");
        }
    }

    #[test]
    fn a_snapshot_stored_before_its_issuer_was_kept_reads_as_issued_by_nobody_known() {
        assert_eq!(
            (codec().event)(json!({ "Snapshotted": {
                "to": "ada@example.com",
                "issued_at": "1970-01-01T00:16:40Z",
                "until": "1970-01-01T00:33:20Z",
                "outcome": "open",
            } })),
            Some(InvitationEvent::Snapshotted {
                to: ada(),
                scope: Scope::Instance,
                by_an_operator: false,
                issued_by: None,
                issued_at: OffsetDateTime::UNIX_EPOCH + Duration::seconds(1_000),
                until: OffsetDateTime::UNIX_EPOCH + Duration::seconds(2_000),
                outcome: Outcome::Open,
                settled_at: None,
                retired: false,
            })
        );
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
