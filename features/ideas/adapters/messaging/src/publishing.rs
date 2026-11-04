use eventsourcing::Recorded;
use ideas_contract::IdeaEventDTO;
use ideas_core::IdeaEvent;
use messaging::Message;

pub fn message_for(happened: &Recorded<IdeaEvent>) -> Option<Message> {
    eventpublishing::message_for(happened, body)
}

fn body(event: &IdeaEvent) -> Option<IdeaEventDTO> {
    Some(match event {
        IdeaEvent::Captured { project, title } => IdeaEventDTO::Captured {
            project: project.to_string(),
            title: title.to_string(),
        },
        IdeaEvent::Retitled(title) => IdeaEventDTO::Retitled {
            title: title.to_string(),
        },
        IdeaEvent::Discarded => IdeaEventDTO::Discarded,
        IdeaEvent::Snapshotted { .. } => return None,
    })
}

#[cfg(test)]
mod tests {
    use eventsourcing::{
        AccountId, Agent, AggregateContext, AggregateId, Event, EventMetadata, Recorded, Version,
    };
    use ideas_contract::{CAPTURED, DISCARDED, EVERY_IDEA, MORE_TO_SWEEP, RETITLED};
    use ideas_core::{IdeaId, IdeaTitle, KIND, ProjectLink};
    use inbox::KeptHandler;
    use messaging::{RoutingKey, Subscription};
    use time::{Duration, OffsetDateTime};

    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn an_idea() -> IdeaId {
        IdeaId::generate(at(1_000))
    }

    fn a_title() -> IdeaTitle {
        IdeaTitle::new("The Loom").expect("a plain title is fine")
    }

    fn recorded(id: &IdeaId, event: IdeaEvent) -> Recorded<IdeaEvent> {
        Recorded {
            metadata: EventMetadata {
                aggregate: AggregateId::from(id),
                kind: KIND,
                version: Version::of(1),
                agent: Agent::Account(AccountId::from("author-7")),
                occurred_at: at(1_000),
                is_snapshot: event.is_snapshot(),
                context: AggregateContext::default(),
            },
            event,
        }
    }

    fn a_capture() -> IdeaEvent {
        IdeaEvent::Captured {
            project: ProjectLink::from("project_1"),
            title: a_title(),
        }
    }

    fn everything_worth_publishing() -> Vec<(IdeaEvent, &'static str)> {
        vec![
            (a_capture(), CAPTURED),
            (IdeaEvent::Retitled(a_title()), RETITLED),
            (IdeaEvent::Discarded, DISCARDED),
        ]
    }

    fn published(id: &IdeaId, event: IdeaEvent) -> Message {
        message_for(&recorded(id, event)).expect("this event should be published")
    }

    #[test]
    fn every_event_lands_on_the_routing_key_the_contract_declares() {
        // given: an idea
        let id = an_idea();

        // then: every event is published on the key the contract declares
        for (event, declared) in everything_worth_publishing() {
            let name = event.name();

            assert_eq!(
                eventpublishing::routing_for(KIND, name).to_string(),
                declared,
                "the key the library derives and the key subscribers bind to must not drift"
            );
            assert_eq!(published(&id, event).routing.to_string(), declared);
        }
    }

    #[test]
    fn the_subscription_the_contract_declares_is_the_one_the_library_derives() {
        // expect: the contract declares the subscription the library derives
        assert_eq!(eventpublishing::everything_from(KIND), EVERY_IDEA);
        // and: it covers every published event
        assert!(
            everything_worth_publishing().into_iter().all(|(event, _)| {
                Subscription::parse(EVERY_IDEA)
                    .expect("a declared pattern")
                    .covers(&eventpublishing::routing_for(KIND, event.name()))
            }),
            "a listener asking for every idea must handle all of them"
        );
    }

    #[test]
    fn the_list_hears_every_idea_event_and_not_the_sweep_s_continuation() {
        // given: what the list listens to
        let hears = |key: &RoutingKey| {
            crate::CatalogueIdeasListener::LISTENS_TO
                .iter()
                .any(|pattern| {
                    Subscription::parse(pattern)
                        .expect("a declared pattern")
                        .covers(key)
                })
        };

        // then: it hears every idea event
        assert!(
            everything_worth_publishing()
                .into_iter()
                .all(|(event, _)| hears(&eventpublishing::routing_for(KIND, event.name()))),
            "a change the list does not hear never reaches it"
        );
        // and: not the sweep's continuation
        assert!(
            !hears(&RoutingKey::parse(MORE_TO_SWEEP).expect("a declared key")),
            "a continuation names no idea, so the list could only refuse it until it is dead"
        );
    }

    #[test]
    fn what_an_idea_event_says_comes_from_the_features_own_mapping() {
        // given: an idea
        let id = an_idea();

        // when: its capture is published
        let body = eventpublishing::published_in::<IdeaEventDTO>(&published(&id, a_capture()))
            .expect("what we wrote must be readable");

        // then: the body says what the feature's mapping says
        assert_eq!(
            body.event.body,
            IdeaEventDTO::Captured {
                project: "project_1".to_owned(),
                title: "The Loom".to_owned(),
            }
        );
    }

    #[test]
    fn the_name_on_the_wire_is_the_name_of_the_event() {
        // given: an idea
        let id = an_idea();

        // then: every event goes out under its own name
        for (event, _) in everything_worth_publishing() {
            let expected = event.name().as_str().to_owned();

            assert_eq!(
                published(&id, event).payload["event"]["name"],
                expected,
                "the contract's serde tags and the aggregate's event names must not drift apart"
            );
        }
    }

    #[test]
    fn a_snapshot_is_not_published_at_all() {
        // given: an idea
        let id = an_idea();
        // and: a snapshot of it
        let snapshot = IdeaEvent::Snapshotted {
            project: ProjectLink::from("project_1"),
            title: a_title(),
            discarded: false,
        };

        // then: the snapshot is not published
        assert!(
            message_for(&recorded(&id, snapshot)).is_none(),
            "collapsing the log is our own housekeeping and no subscriber's business"
        );
    }

    #[test]
    fn the_idea_can_be_read_back_out_of_a_message() {
        // given: an idea
        let id = an_idea();

        // then: the idea is read back out of its published message
        assert_eq!(
            eventpublishing::aggregate_in::<IdeaId>(&published(&id, a_capture()))
                .expect("what we wrote must be readable"),
            id
        );
    }
}
