use std::error::Error as AnyError;
use std::str::FromStr;

use clock::text;
use eventsourcing::{AggregateType, Event, EventName, Recorded};
use messaging::{Message, RoutingKey};
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::Value;
use thiserror::Error;

use crate::published::{PublishedAgent, PublishedAggregate, PublishedBody, PublishedEvent};

const SEPARATOR: &str = ".";

#[derive(Debug, Error)]
pub enum UnreadableMessage {
    #[error("this message does not carry a published event")]
    NotAPublishedEvent(#[source] serde_json::Error),
    #[error("this message names its aggregate by something that is not its id")]
    NotItsId(#[source] Box<dyn AnyError + Send + Sync>),
}

pub fn routing_for(kind: AggregateType, name: EventName) -> RoutingKey {
    let parts = name.as_str().to_ascii_lowercase().replace('_', SEPARATOR);

    RoutingKey::parse(&format!("{}{SEPARATOR}{parts}", kind.as_str()))
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
        occurred_at: text::serialize(happened.metadata.occurred_at),
        context: happened.metadata.context.entries().clone(),
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

pub fn aggregate_in<Id>(message: &Message) -> Result<Id, UnreadableMessage>
where
    Id: FromStr,
    Id::Err: AnyError + Send + Sync + 'static,
{
    published_in::<Value>(message)?
        .aggregate
        .id
        .parse()
        .map_err(|why| UnreadableMessage::NotItsId(Box::new(why)))
}
