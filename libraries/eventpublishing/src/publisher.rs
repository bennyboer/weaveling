use std::sync::Arc;

use eventsourcing::{AggregateType, Event, EventName, Recorded};
use messaging::{Message, Publisher, RoutingKey, Undelivered};
use serde::Serialize;
use serde::de::DeserializeOwned;
use thiserror::Error;

use crate::published::{
    PublishedAgent, PublishedAggregate, PublishedBody, PublishedEvent, as_rfc3339,
};

const SEPARATOR: char = '.';

pub struct EventPublisher<E, B> {
    publisher: Arc<dyn Publisher>,
    body: fn(&E) -> Option<B>,
}

#[derive(Debug, Error)]
pub enum UnreadableMessage {
    #[error("this message does not carry a published event")]
    NotAPublishedEvent(#[source] serde_json::Error),
}

pub fn routing_for(kind: AggregateType, name: EventName) -> RoutingKey {
    let as_rfc3339 = name.as_str().to_ascii_lowercase().replace('_', "-");

    RoutingKey::parse(&format!("{}{SEPARATOR}{as_rfc3339}", kind.as_str()))
        .expect("an aggregate kind and an event name hold no wildcards")
}

pub fn everything_from(kind: AggregateType) -> String {
    format!("{}{SEPARATOR}#", kind.as_str())
}

pub fn message_for<E, B>(happened: &Recorded<E>, body: fn(&E) -> Option<B>) -> Option<Message>
where
    E: Event,
    B: Serialize,
{
    if happened.metadata.is_snapshot {
        return None;
    }

    let published = PublishedEvent {
        event: PublishedBody {
            version: happened.event.version().count(),
            body: body(&happened.event)?,
        },
        aggregate: PublishedAggregate::of(&happened.metadata),
        agent: PublishedAgent::of(&happened.metadata.agent),
        occurred_at: as_rfc3339(happened.metadata.occurred_at),
    };
    let payload = serde_json::to_value(published)
        .expect("a published event is plain data and cannot fail to serialize");

    Some(Message::opening(
        routing_for(happened.metadata.kind, happened.event.name()),
        payload,
        happened.metadata.occurred_at,
    ))
}

pub fn published_in<B: DeserializeOwned>(
    message: &Message,
) -> Result<PublishedEvent<B>, UnreadableMessage> {
    serde_json::from_value(message.payload.clone()).map_err(UnreadableMessage::NotAPublishedEvent)
}

impl<E, B> EventPublisher<E, B>
where
    E: Event,
    B: Serialize,
{
    pub fn new(publisher: Arc<dyn Publisher>, body: fn(&E) -> Option<B>) -> Self {
        Self { publisher, body }
    }

    pub fn message_for(&self, happened: &Recorded<E>) -> Option<Message> {
        message_for(happened, self.body)
    }

    pub async fn publish(&self, happened: &Recorded<E>) -> Result<(), Undelivered> {
        let Some(message) = self.message_for(happened) else {
            return Ok(());
        };

        self.publisher.publish(message).await
    }
}
