use clock::text;
use eventsourcing::Recorded;
use messaging::Message;
use readings_contract::{ExtentDTO, ReadingCopyEventDTO};
use readings_core::{Extent, ReadingCopyEvent};

/// The message a reading copy's event goes out as; a snapshot goes out as none.
pub fn message_for(happened: &Recorded<ReadingCopyEvent>) -> Option<Message> {
    eventpublishing::message_for(happened, body)
}

fn body(event: &ReadingCopyEvent) -> Option<ReadingCopyEventDTO> {
    Some(match event {
        ReadingCopyEvent::Made {
            name,
            extent,
            times,
            ..
        } => ReadingCopyEventDTO::Made {
            name: name.to_string(),
            extent: to_extent_dto(*extent),
            open_until: times.open_until.map(text::serialize),
            deleted_on: times.deleted_on.map(text::serialize),
        },
        ReadingCopyEvent::Renamed(name) => ReadingCopyEventDTO::Renamed {
            name: name.to_string(),
        },
        ReadingCopyEvent::Extended(times) => ReadingCopyEventDTO::Extended {
            open_until: times.open_until.map(text::serialize),
            deleted_on: times.deleted_on.map(text::serialize),
        },
        ReadingCopyEvent::Deleted => ReadingCopyEventDTO::Deleted,
        ReadingCopyEvent::Snapshotted { .. } => return None,
    })
}

fn to_extent_dto(extent: Extent) -> ExtentDTO {
    match extent {
        Extent::Book => ExtentDTO::Book,
        Extent::Sections(count) => ExtentDTO::Sections { count },
        Extent::Scenes(count) => ExtentDTO::Scenes { count },
    }
}

#[cfg(test)]
mod tests {
    use eventsourcing::{
        AccountId, Agent, AggregateContext, AggregateId, Event, EventMetadata, Version,
    };
    use readings_contract::{DELETED, EVERY_READING_COPY, EXTENDED, MADE, RENAMED};
    use readings_core::{KIND, ProjectLink, ReadingCopyId, ReadingCopyName, Times};
    use time::{Duration, OffsetDateTime};

    use super::*;

    const AT: OffsetDateTime = OffsetDateTime::UNIX_EPOCH;

    fn a_copy() -> ReadingCopyId {
        ReadingCopyId::generate(AT)
    }

    fn a_name(saying: &str) -> ReadingCopyName {
        ReadingCopyName::new(saying).expect("a plain name is fine")
    }

    fn recorded(id: &ReadingCopyId, event: ReadingCopyEvent) -> Recorded<ReadingCopyEvent> {
        Recorded {
            metadata: EventMetadata {
                aggregate: AggregateId::from(id),
                kind: KIND,
                version: Version::of(1),
                agent: Agent::Account(AccountId::from("account_hedy")),
                occurred_at: AT,
                is_snapshot: event.is_snapshot(),
                context: AggregateContext::default(),
            },
            event,
        }
    }

    fn published(event: ReadingCopyEvent) -> Message {
        message_for(&recorded(&a_copy(), event)).expect("this event should be published")
    }

    fn times() -> Times {
        Times {
            open_until: Some(AT + Duration::days(30)),
            deleted_on: None,
        }
    }

    fn everything_worth_publishing() -> Vec<(ReadingCopyEvent, &'static str)> {
        vec![
            (
                ReadingCopyEvent::Made {
                    project: ProjectLink::from("project_loom"),
                    name: a_name("For Lena"),
                    extent: Extent::Sections(3),
                    times: times(),
                },
                MADE,
            ),
            (
                ReadingCopyEvent::Renamed(a_name("For Lena, again")),
                RENAMED,
            ),
            (ReadingCopyEvent::Extended(times()), EXTENDED),
            (ReadingCopyEvent::Deleted, DELETED),
        ]
    }

    #[test]
    fn every_event_lands_on_the_routing_key_the_contract_declares() {
        // expect: every event lands on the key its contract declares, and the subscription to all of them covers it
        assert_eq!(eventpublishing::everything_from(KIND), EVERY_READING_COPY);
        for (event, declared) in everything_worth_publishing() {
            assert_eq!(
                eventpublishing::routing_for(KIND, event.name()).to_string(),
                declared,
                "the key the library derives and the key subscribers bind to must not drift"
            );
            assert_eq!(published(event).routing.to_string(), declared);
        }
    }

    #[test]
    fn the_name_on_the_wire_is_the_name_of_the_event() {
        // expect: every message is named as its event
        for (event, _) in everything_worth_publishing() {
            let expected = event.name().as_str().to_owned();

            assert_eq!(
                published(event).payload["event"]["name"],
                expected,
                "the contract's serde tags and the aggregate's event names must not drift apart"
            );
        }
    }

    #[test]
    fn a_made_copy_carries_its_name_what_it_holds_and_its_times() {
        // when: a copy made is published
        let told = eventpublishing::published_in::<ReadingCopyEventDTO>(&published(
            everything_worth_publishing().remove(0).0,
        ))
        .expect("what we wrote must be readable");

        // then: it says what it is called, holds, and how long it is open and kept
        assert_eq!(
            told.event.body,
            ReadingCopyEventDTO::Made {
                name: "For Lena".to_owned(),
                extent: ExtentDTO::Sections { count: 3 },
                open_until: Some("1970-01-31T00:00:00Z".to_owned()),
                deleted_on: None,
            }
        );
    }

    #[test]
    fn a_snapshot_is_not_published_at_all() {
        // given: a snapshot of a copy
        let snapshot = ReadingCopyEvent::Snapshotted {
            project: ProjectLink::from("project_loom"),
            name: a_name("For Lena"),
            extent: Extent::Book,
            times: times(),
            made_by: Agent::Local,
            made_at: AT,
            deleted: false,
        };

        // then: it makes no message
        assert!(
            message_for(&recorded(&a_copy(), snapshot)).is_none(),
            "collapsing the log is our own housekeeping and no subscriber's business"
        );
    }
}
