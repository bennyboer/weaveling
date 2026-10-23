use eventsourcing::Recorded;
use invitations_contract::InvitationEventDTO;
use invitations_core::InvitationEvent;
use messaging::Message;

pub fn message_for(happened: &Recorded<InvitationEvent>) -> Option<Message> {
    eventpublishing::message_for(happened, body)
}

fn body(event: &InvitationEvent) -> Option<InvitationEventDTO> {
    Some(match event {
        InvitationEvent::Issued { .. } => InvitationEventDTO::Issued,
        InvitationEvent::Accepted => InvitationEventDTO::Accepted,
        InvitationEvent::Withdrawn => InvitationEventDTO::Withdrawn,
        InvitationEvent::Snapshotted { .. } => return None,
    })
}

#[cfg(test)]
mod tests {
    use email::EmailAddress;
    use eventsourcing::{Agent, AggregateId, Event, EventMetadata, Version};
    use invitations_contract::{ACCEPTED, EVERY_INVITATION, ISSUED, WITHDRAWN};
    use invitations_core::{InvitationId, KIND, Outcome};
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
            },
            event,
        }
    }

    fn everything_worth_publishing() -> Vec<(InvitationEvent, &'static str)> {
        vec![
            (InvitationEvent::Issued { to: ada() }, ISSUED),
            (InvitationEvent::Accepted, ACCEPTED),
            (InvitationEvent::Withdrawn, WITHDRAWN),
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
    fn the_address_invited_never_travels_with_a_message() {
        let id = InvitationId::generate(at(1_000));

        for (event, _) in everything_worth_publishing() {
            let told = message_for(&recorded(&id, event))
                .expect("published")
                .payload
                .to_string();

            assert!(
                !told.contains("ada@example.com"),
                "messages are kept and refused ones shown, so the address must not be in {told}"
            );
        }
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
                    issued_at: at(1_000),
                    outcome: Outcome::Open,
                }
            ))
            .is_none()
        );
    }
}
