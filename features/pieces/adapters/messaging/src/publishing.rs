use std::sync::Arc;

use async_trait::async_trait;
use eventpublishing::{
    EventPublisher, PublishedEvent, UnreadableMessage, message_for as message_carrying,
    published_in,
};
use eventsourcing::Recorded;
use ids::InvalidId;
use messaging::{Message, Publisher, Subscription};
use pieces_contract::{EVERY_PIECE, PieceEventDTO};
use pieces_core::{PieceEvent, PieceEventPublisher, PieceId, PublishError};
use thiserror::Error;

pub struct Publishing {
    publishing: EventPublisher<PieceEvent, PieceEventDTO>,
}

#[derive(Debug, Error)]
pub enum UnreadablePieceEvent {
    #[error(transparent)]
    NotAPieceEvent(#[from] UnreadableMessage),
    #[error("this message names something that is not a piece")]
    NotAPieceId(#[source] InvalidId),
}

pub fn every_piece() -> Subscription {
    Subscription::parse(EVERY_PIECE).expect("the piece pattern is written at compile time")
}

pub fn event_in(message: &Message) -> Result<PublishedEvent<PieceEventDTO>, UnreadablePieceEvent> {
    Ok(published_in(message)?)
}

pub fn piece_in(message: &Message) -> Result<PieceId, UnreadablePieceEvent> {
    event_in(message)?
        .aggregate
        .id
        .parse()
        .map_err(UnreadablePieceEvent::NotAPieceId)
}

pub fn message_for(happened: &Recorded<PieceEvent>) -> Option<Message> {
    message_carrying(happened, body)
}

impl Publishing {
    pub fn new(publisher: Arc<dyn Publisher>) -> Self {
        Self {
            publishing: EventPublisher::new(publisher, body),
        }
    }
}

#[async_trait]
impl PieceEventPublisher for Publishing {
    async fn publish(&self, happened: &Recorded<PieceEvent>) -> Result<(), PublishError> {
        self.publishing
            .publish(happened)
            .await
            .map_err(PublishError::because)
    }
}

fn body(event: &PieceEvent) -> Option<PieceEventDTO> {
    Some(match event {
        PieceEvent::Captured { project, title } => PieceEventDTO::Captured {
            project: project.to_string(),
            title: title.to_string(),
        },
        PieceEvent::Retitled(title) => PieceEventDTO::Retitled {
            title: title.to_string(),
        },
        PieceEvent::PassageAttached { passage } => PieceEventDTO::PassageAttached {
            passage: passage.to_string(),
        },
        PieceEvent::Discarded => PieceEventDTO::Discarded,
        PieceEvent::Snapshotted { .. } => return None,
    })
}

#[cfg(test)]
mod tests {
    use eventpublishing::{everything_from, routing_for};
    use eventsourcing::{Agent, AgentId, AggregateId, Event, EventMetadata, Recorded, Version};
    use messaging::RoutingKey;
    use pieces_contract::{CAPTURED, DISCARDED, PASSAGE_ATTACHED, RETITLED};
    use pieces_core::{KIND, PassageLink, PieceTitle, ProjectLink};
    use serde_json::json;
    use time::{Duration, OffsetDateTime};

    use super::*;

    fn at(seconds: i64) -> OffsetDateTime {
        OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
    }

    fn a_piece() -> PieceId {
        PieceId::generate(at(1_000))
    }

    fn a_title() -> PieceTitle {
        PieceTitle::new("The Loom").expect("a plain title is fine")
    }

    fn recorded(id: &PieceId, event: PieceEvent) -> Recorded<PieceEvent> {
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

    fn a_capture() -> PieceEvent {
        PieceEvent::Captured {
            project: ProjectLink::from("project_1"),
            title: a_title(),
        }
    }

    fn everything_worth_publishing() -> Vec<(PieceEvent, &'static str)> {
        vec![
            (a_capture(), CAPTURED),
            (PieceEvent::Retitled(a_title()), RETITLED),
            (
                PieceEvent::PassageAttached {
                    passage: PassageLink::from("passage_1"),
                },
                PASSAGE_ATTACHED,
            ),
            (PieceEvent::Discarded, DISCARDED),
        ]
    }

    fn published(id: &PieceId, event: PieceEvent) -> Message {
        message_for(&recorded(id, event)).expect("this event should be published")
    }

    #[test]
    fn every_event_lands_on_the_routing_key_the_contract_declares() {
        let id = a_piece();

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
        assert_eq!(everything_from(KIND), EVERY_PIECE);
        assert!(
            everything_worth_publishing()
                .into_iter()
                .all(|(event, _)| { every_piece().covers(&routing_for(KIND, event.name())) }),
            "a listener asking for every piece must handle all of them"
        );
    }

    #[test]
    fn what_a_piece_event_says_comes_from_the_features_own_mapping() {
        let id = a_piece();

        let body = event_in(&published(&id, a_capture())).expect("what we wrote must be readable");

        assert_eq!(
            body.event.body,
            PieceEventDTO::Captured {
                project: "project_1".to_owned(),
                title: "The Loom".to_owned(),
            }
        );
    }

    #[test]
    fn the_name_on_the_wire_is_the_name_of_the_event() {
        let id = a_piece();

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
        let id = a_piece();
        let snapshot = PieceEvent::Snapshotted {
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
    fn the_piece_can_be_read_back_out_of_a_message() {
        let id = a_piece();

        assert_eq!(
            piece_in(&published(&id, a_capture())).expect("what we wrote must be readable"),
            id
        );
    }

    #[test]
    fn a_message_about_something_else_entirely_is_refused() {
        let stray = Message::opening(
            RoutingKey::parse("piece.captured").expect("a plain key is fine"),
            json!({ "nothing": "useful" }),
            at(1_000),
        );

        assert!(matches!(
            event_in(&stray),
            Err(UnreadablePieceEvent::NotAPieceEvent(..))
        ));
    }
}
