use std::sync::Arc;

use async_trait::async_trait;
use eventpublishing::{
    MessagingEventPublisher, PublishedEvent, UnreadableMessage, message_for as message_carrying,
    published_in,
};
use eventsourcing::{EventPublisher, PublishError, Recorded};
use ideas_contract::{EVERY_IDEA, IdeaEventDTO};
use ideas_core::{IdeaEvent, IdeaId};
use ids::InvalidId;
use messaging::{Message, Publisher, Subscription};
use thiserror::Error;

pub struct IdeaEventPublisher {
    publishing: MessagingEventPublisher<IdeaEvent, IdeaEventDTO>,
}

#[derive(Debug, Error)]
pub enum UnreadableIdeaEvent {
    #[error(transparent)]
    NotAIdeaEvent(#[from] UnreadableMessage),
    #[error("this message names something that is not a idea")]
    NotAIdeaId(#[source] InvalidId),
}

pub fn every_event() -> Subscription {
    Subscription::parse(EVERY_IDEA).expect("the idea pattern is written at compile time")
}

pub fn event_in(message: &Message) -> Result<PublishedEvent<IdeaEventDTO>, UnreadableIdeaEvent> {
    Ok(published_in(message)?)
}

pub fn idea_in(message: &Message) -> Result<IdeaId, UnreadableIdeaEvent> {
    event_in(message)?
        .aggregate
        .id
        .parse()
        .map_err(UnreadableIdeaEvent::NotAIdeaId)
}

pub fn message_for(happened: &Recorded<IdeaEvent>) -> Option<Message> {
    message_carrying(happened, body)
}

impl IdeaEventPublisher {
    pub fn new(publisher: Arc<dyn Publisher>) -> Self {
        Self {
            publishing: MessagingEventPublisher::new(publisher, body),
        }
    }
}

#[async_trait]
impl EventPublisher<IdeaEvent> for IdeaEventPublisher {
    async fn publish(&self, happened: &Recorded<IdeaEvent>) -> Result<(), PublishError> {
        self.publishing
            .publish(happened)
            .await
            .map_err(PublishError::because)
    }
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
        IdeaEvent::PassageAttached { passage } => IdeaEventDTO::PassageAttached {
            passage: passage.to_string(),
        },
        IdeaEvent::Discarded { passage } => IdeaEventDTO::Discarded {
            passage: passage.as_ref().map(ToString::to_string),
        },
        IdeaEvent::Snapshotted { .. } => return None,
    })
}

#[cfg(test)]
mod tests {
    use eventpublishing::{everything_from, routing_for};
    use eventsourcing::{Agent, AgentId, AggregateId, Event, EventMetadata, Recorded, Version};
    use ideas_contract::{CAPTURED, DISCARDED, PASSAGE_ATTACHED, RETITLED};
    use ideas_core::{IdeaTitle, KIND, PassageLink, ProjectLink};
    use messaging::RoutingKey;
    use serde_json::json;
    use time::{Duration, OffsetDateTime};

    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn a_idea() -> IdeaId {
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
                agent: Agent::User(AgentId::from("author-7")),
                occurred_at: at(1_000),
                is_snapshot: event.is_snapshot(),
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
            (
                IdeaEvent::PassageAttached {
                    passage: PassageLink::from("passage_1"),
                },
                PASSAGE_ATTACHED,
            ),
            (IdeaEvent::Discarded { passage: None }, DISCARDED),
        ]
    }

    fn published(id: &IdeaId, event: IdeaEvent) -> Message {
        message_for(&recorded(id, event)).expect("this event should be published")
    }

    #[test]
    fn every_event_lands_on_the_routing_key_the_contract_declares() {
        let id = a_idea();

        for (event, declared) in everything_worth_publishing() {
            let name = event.name();

            assert_eq!(
                routing_for(KIND, name).to_string(),
                declared,
                "the key the library derives and the key subscribers bind to must not drift"
            );
            assert_eq!(published(&id, event).routing.to_string(), declared);
        }
    }

    #[test]
    fn the_subscription_the_contract_declares_is_the_one_the_library_derives() {
        assert_eq!(everything_from(KIND), EVERY_IDEA);
        assert!(
            everything_worth_publishing()
                .into_iter()
                .all(|(event, _)| { every_event().covers(&routing_for(KIND, event.name())) }),
            "a listener asking for every idea must handle all of them"
        );
    }

    #[test]
    fn what_a_idea_event_says_comes_from_the_features_own_mapping() {
        let id = a_idea();

        let body = event_in(&published(&id, a_capture())).expect("what we wrote must be readable");

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
        let id = a_idea();

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
        let id = a_idea();
        let snapshot = IdeaEvent::Snapshotted {
            project: ProjectLink::from("project_1"),
            title: a_title(),
            passage: Some(PassageLink::from("passage_1")),
            discarded: false,
        };

        assert!(
            message_for(&recorded(&id, snapshot)).is_none(),
            "collapsing the log is our own housekeeping and no subscriber's business"
        );
    }

    #[test]
    fn the_idea_can_be_read_back_out_of_a_message() {
        let id = a_idea();

        assert_eq!(
            idea_in(&published(&id, a_capture())).expect("what we wrote must be readable"),
            id
        );
    }

    #[test]
    fn a_message_about_something_else_entirely_is_refused() {
        let stray = Message::opening(
            RoutingKey::parse("idea.captured").expect("a plain key is fine"),
            json!({ "nothing": "useful" }),
            at(1_000),
        );

        assert!(matches!(
            event_in(&stray),
            Err(UnreadableIdeaEvent::NotAIdeaEvent(..))
        ));
    }

    #[test]
    fn a_discard_puts_the_passage_on_the_wire() {
        let id = a_idea();

        let body = event_in(&published(
            &id,
            IdeaEvent::Discarded {
                passage: Some(PassageLink::from("passage_1")),
            },
        ))
        .expect("what we wrote must be readable");

        assert_eq!(
            body.event.body,
            IdeaEventDTO::Discarded {
                passage: Some("passage_1".to_owned()),
            },
            "the passage has to survive the mapping, because the listener that deletes the              prose has no other way of learning which passage it was"
        );
    }
}
