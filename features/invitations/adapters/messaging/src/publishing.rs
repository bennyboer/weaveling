use clock::text;
use eventsourcing::Recorded;
use invitations_contract::{InvitationEventDTO, ScopeDTO, decline_reason, role};
use invitations_core::{DeclineReason, InvitationEvent, Role, Scope};
use messaging::Message;

pub fn message_for(happened: &Recorded<InvitationEvent>) -> Option<Message> {
    eventpublishing::message_for(happened, body)
}

fn body(event: &InvitationEvent) -> Option<InvitationEventDTO> {
    Some(match event {
        InvitationEvent::Issued {
            to,
            digest,
            until,
            scope,
            by_an_operator,
        } => InvitationEventDTO::Issued {
            to: to.to_string(),
            digest: digest.as_str().to_owned(),
            until: text::serialize(*until),
            scope: to_scope_dto(scope),
            by_an_operator: *by_an_operator,
        },
        InvitationEvent::Accepted { scope } => InvitationEventDTO::Accepted {
            scope: to_scope_dto(scope),
        },
        InvitationEvent::Withdrawn => InvitationEventDTO::Withdrawn,
        InvitationEvent::Declined { reason } => InvitationEventDTO::Declined {
            reason: declined_for(*reason).to_owned(),
        },
        InvitationEvent::Retired => InvitationEventDTO::Retired,
        InvitationEvent::Snapshotted { .. } => return None,
    })
}

fn to_scope_dto(scope: &Scope) -> ScopeDTO {
    match scope {
        Scope::Instance => ScopeDTO::Instance,
        Scope::Project { project, role } => ScopeDTO::Project {
            project: project.to_string(),
            role: named(*role).to_owned(),
        },
    }
}

fn named(given: Role) -> &'static str {
    match given {
        Role::Owner => role::OWNER,
        Role::Author => role::AUTHOR,
        Role::Reader => role::READER,
    }
}

fn declined_for(reason: DeclineReason) -> &'static str {
    match reason {
        DeclineReason::AddressHasAccount => decline_reason::ADDRESS_HAS_ACCOUNT,
        DeclineReason::TurnedDown => decline_reason::TURNED_DOWN,
    }
}

#[cfg(test)]
mod tests {
    use email::EmailAddress;
    use eventsourcing::{Agent, AggregateContext, AggregateId, Event, EventMetadata, Version};
    use invitations_contract::{ACCEPTED, DECLINED, EVERY_INVITATION, ISSUED, RETIRED, WITHDRAWN};
    use invitations_core::{InvitationId, InvitationTokenDigest, KIND, Outcome, ProjectLink};
    use messaging::Subscription;
    use time::{Duration, OffsetDateTime};

    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn ada() -> EmailAddress {
        EmailAddress::new("ada@example.com").expect("a plain address")
    }

    fn recorded(id: &InvitationId, event: InvitationEvent) -> Recorded<InvitationEvent> {
        Recorded {
            metadata: EventMetadata {
                aggregate: AggregateId::from(id),
                kind: KIND,
                version: Version::of(1),
                agent: Agent::Account("account_1".into()),
                occurred_at: at(1_000),
                is_snapshot: event.is_snapshot(),
                context: AggregateContext::default(),
            },
            event,
        }
    }

    fn an_issue() -> InvitationEvent {
        InvitationEvent::Issued {
            to: ada(),
            digest: InvitationTokenDigest::kept("ab12".to_owned()),
            until: at(1_000) + Duration::days(7),
            scope: Scope::Instance,
            by_an_operator: true,
        }
    }

    fn everything_worth_publishing() -> Vec<(InvitationEvent, &'static str)> {
        vec![
            (an_issue(), ISSUED),
            (
                InvitationEvent::Accepted {
                    scope: Scope::Instance,
                },
                ACCEPTED,
            ),
            (InvitationEvent::Withdrawn, WITHDRAWN),
            (
                InvitationEvent::Declined {
                    reason: DeclineReason::AddressHasAccount,
                },
                DECLINED,
            ),
            (InvitationEvent::Retired, RETIRED),
        ]
    }

    #[test]
    fn every_event_lands_on_the_routing_key_the_contract_declares() {
        let id = InvitationId::generate(at(1_000));

        for (event, declared) in everything_worth_publishing() {
            assert_eq!(
                eventpublishing::routing_for(KIND, event.name()).to_string(),
                declared
            );
            let message = message_for(&recorded(&id, event)).expect("published");
            assert_eq!(message.routing.to_string(), declared);
            assert!(
                Subscription::parse(EVERY_INVITATION)
                    .expect("a declared pattern")
                    .covers(&message.routing)
            );
        }
        assert_eq!(eventpublishing::everything_from(KIND), EVERY_INVITATION);
    }

    #[test]
    fn an_issue_tells_to_whom_by_what_digest_and_until_when_and_nothing_else_tells_the_address() {
        let id = InvitationId::generate(at(1_000));

        let issued = eventsourcing_body(&recorded(&id, an_issue()));
        assert_eq!(
            issued,
            InvitationEventDTO::Issued {
                to: "ada@example.com".to_owned(),
                digest: "ab12".to_owned(),
                until: "1970-01-08T00:16:40Z".to_owned(),
                scope: ScopeDTO::Instance,
                by_an_operator: true,
            },
            "accounts keep their tickets from it, the token's digest being harmless and the address judged so"
        );
        for event in [
            InvitationEvent::Accepted {
                scope: Scope::Instance,
            },
            InvitationEvent::Withdrawn,
            InvitationEvent::Declined {
                reason: DeclineReason::AddressHasAccount,
            },
        ] {
            let told = message_for(&recorded(&id, event))
                .expect("published")
                .payload
                .to_string();
            assert!(!told.contains("ada@example.com"), "{told}");
        }
    }

    #[test]
    fn an_acceptance_tells_into_which_project_as_what() {
        let id = InvitationId::generate(at(1_000));

        let accepted = eventsourcing_body(&recorded(
            &id,
            InvitationEvent::Accepted {
                scope: Scope::Project {
                    project: ProjectLink::from("project_1"),
                    role: Role::Author,
                },
            },
        ));

        assert_eq!(
            accepted,
            InvitationEventDTO::Accepted {
                scope: ScopeDTO::Project {
                    project: "project_1".to_owned(),
                    role: "author".to_owned(),
                },
            },
            "members let whoever accepted join from it alone, the account being its agent"
        );
    }

    #[test]
    fn an_issue_into_a_project_tells_which_as_what_and_whether_an_operator_issued_it() {
        let id = InvitationId::generate(at(1_000));

        for (role, named) in [
            (Role::Owner, "owner"),
            (Role::Author, "author"),
            (Role::Reader, "reader"),
        ] {
            let issued = eventsourcing_body(&recorded(
                &id,
                InvitationEvent::Issued {
                    to: ada(),
                    digest: InvitationTokenDigest::kept("ab12".to_owned()),
                    until: at(1_000),
                    scope: Scope::Project {
                        project: ProjectLink::from("project_1"),
                        role,
                    },
                    by_an_operator: false,
                },
            ));

            assert_eq!(
                issued,
                InvitationEventDTO::Issued {
                    to: "ada@example.com".to_owned(),
                    digest: "ab12".to_owned(),
                    until: "1970-01-01T00:16:40Z".to_owned(),
                    scope: ScopeDTO::Project {
                        project: "project_1".to_owned(),
                        role: named.to_owned(),
                    },
                    by_an_operator: false,
                }
            );
        }
    }

    #[test]
    fn a_decline_tells_why() {
        let id = InvitationId::generate(at(1_000));

        for (reason, told) in [
            (DeclineReason::AddressHasAccount, "address-has-account"),
            (DeclineReason::TurnedDown, "turned-down"),
        ] {
            assert_eq!(
                eventsourcing_body(&recorded(&id, InvitationEvent::Declined { reason })),
                InvitationEventDTO::Declined {
                    reason: told.to_owned()
                }
            );
        }
    }

    fn eventsourcing_body(happened: &Recorded<InvitationEvent>) -> InvitationEventDTO {
        eventpublishing::published_in::<InvitationEventDTO>(
            &message_for(happened).expect("published"),
        )
        .expect("readable")
        .event
        .body
    }

    #[test]
    fn the_name_on_the_wire_is_the_name_of_the_event_and_the_invitation_reads_back() {
        let id = InvitationId::generate(at(1_000));

        for (event, _) in everything_worth_publishing() {
            let expected = event.name().as_str().to_owned();
            let message = message_for(&recorded(&id, event)).expect("published");

            assert_eq!(message.payload["event"]["name"], expected);
            assert_eq!(
                eventpublishing::aggregate_in::<InvitationId>(&message).expect("readable"),
                id
            );
        }
    }

    #[test]
    fn a_snapshot_is_not_published_at_all() {
        let id = InvitationId::generate(at(1_000));

        assert!(
            message_for(&recorded(
                &id,
                InvitationEvent::Snapshotted {
                    to: ada(),
                    scope: Scope::Instance,
                    by_an_operator: true,
                    issued_by: None,
                    issued_at: at(1_000),
                    until: at(1_000) + Duration::days(7),
                    outcome: Outcome::Open,
                    settled_at: None,
                    retired: false,
                }
            ))
            .is_none()
        );
    }
}
