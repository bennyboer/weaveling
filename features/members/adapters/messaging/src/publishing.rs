use eventsourcing::Recorded;
use members_contract::{MembersEventDTO, role};
use members_core::{MembersEvent, Role};
use messaging::Message;

pub fn message_for(happened: &Recorded<MembersEvent>) -> Option<Message> {
    eventpublishing::message_for(happened, body)
}

fn body(event: &MembersEvent) -> Option<MembersEventDTO> {
    Some(match event {
        MembersEvent::Founded { owner } => MembersEventDTO::Founded {
            owner: owner.to_string(),
        },
        MembersEvent::Joined { account, role } => MembersEventDTO::Joined {
            account: account.to_string(),
            role: named(*role).to_owned(),
        },
        MembersEvent::RoleChanged { account, to } => MembersEventDTO::RoleChanged {
            account: account.to_string(),
            to: named(*to).to_owned(),
        },
        MembersEvent::Left { account } => MembersEventDTO::Left {
            account: account.to_string(),
        },
        MembersEvent::Removed { account } => MembersEventDTO::Removed {
            account: account.to_string(),
        },
        MembersEvent::Disbanded => MembersEventDTO::Disbanded,
        MembersEvent::Snapshotted { .. } => return None,
    })
}

fn named(given: Role) -> &'static str {
    match given {
        Role::Owner => role::OWNER,
        Role::Author => role::AUTHOR,
        Role::Reader => role::READER,
    }
}

#[cfg(test)]
mod tests {
    use eventsourcing::{
        AccountId, Agent, AggregateContext, AggregateId, Event, EventMetadata, Version,
    };
    use members_contract::{
        DISBANDED, EVERY_MEMBERS, FOUNDED, JOINED, LEFT, REMOVED, ROLE_CHANGED,
    };
    use members_core::{KIND, Member, ProjectLink};
    use messaging::Subscription;
    use time::OffsetDateTime;

    use super::*;

    fn grace() -> AccountId {
        AccountId::from("account_grace")
    }

    fn recorded(event: MembersEvent) -> Recorded<MembersEvent> {
        Recorded {
            metadata: EventMetadata {
                aggregate: AggregateId::from(&ProjectLink::from("project_1")),
                kind: KIND,
                version: Version::of(1),
                agent: Agent::Account(AccountId::from("account_ada")),
                occurred_at: OffsetDateTime::UNIX_EPOCH,
                is_snapshot: event.is_snapshot(),
                context: AggregateContext::default(),
            },
            event,
        }
    }

    fn everything_worth_publishing() -> Vec<(MembersEvent, &'static str, MembersEventDTO)> {
        vec![
            (
                MembersEvent::Founded { owner: grace() },
                FOUNDED,
                MembersEventDTO::Founded {
                    owner: "account_grace".to_owned(),
                },
            ),
            (
                MembersEvent::Joined {
                    account: grace(),
                    role: Role::Author,
                },
                JOINED,
                MembersEventDTO::Joined {
                    account: "account_grace".to_owned(),
                    role: "author".to_owned(),
                },
            ),
            (
                MembersEvent::RoleChanged {
                    account: grace(),
                    to: Role::Reader,
                },
                ROLE_CHANGED,
                MembersEventDTO::RoleChanged {
                    account: "account_grace".to_owned(),
                    to: "reader".to_owned(),
                },
            ),
            (
                MembersEvent::RoleChanged {
                    account: grace(),
                    to: Role::Owner,
                },
                ROLE_CHANGED,
                MembersEventDTO::RoleChanged {
                    account: "account_grace".to_owned(),
                    to: "owner".to_owned(),
                },
            ),
            (
                MembersEvent::Left { account: grace() },
                LEFT,
                MembersEventDTO::Left {
                    account: "account_grace".to_owned(),
                },
            ),
            (
                MembersEvent::Removed { account: grace() },
                REMOVED,
                MembersEventDTO::Removed {
                    account: "account_grace".to_owned(),
                },
            ),
            (
                MembersEvent::Disbanded,
                DISBANDED,
                MembersEventDTO::Disbanded,
            ),
        ]
    }

    #[test]
    fn every_event_lands_on_its_declared_key_telling_who_and_as_what() {
        for (event, declared, told) in everything_worth_publishing() {
            assert_eq!(
                eventpublishing::routing_for(KIND, event.name()).to_string(),
                declared
            );
            let message = message_for(&recorded(event)).expect("published");
            assert_eq!(message.routing.to_string(), declared);
            assert!(
                Subscription::parse(EVERY_MEMBERS)
                    .expect("a declared pattern")
                    .covers(&message.routing)
            );
            assert_eq!(
                eventpublishing::published_in::<MembersEventDTO>(&message)
                    .expect("readable")
                    .event
                    .body,
                told
            );
            assert_eq!(
                eventpublishing::aggregate_in::<ProjectLink>(&message).expect("readable"),
                ProjectLink::from("project_1")
            );
        }
        assert_eq!(eventpublishing::everything_from(KIND), EVERY_MEMBERS);
    }

    #[test]
    fn a_snapshot_is_not_published_at_all() {
        assert!(
            message_for(&recorded(MembersEvent::Snapshotted {
                members: vec![Member {
                    account: grace(),
                    role: Role::Owner,
                }],
                disbanded: false,
            }))
            .is_none()
        );
    }
}
