use clock::text;
use eventsourcing::Agent;
use messaging::{Message, RoutingKey};
use serde_json::{Value, json};
use time::OffsetDateTime;

use crate::published::{PublishedAgent, PublishedAggregate};

/// A published event as a test hands it to a listener: `event` is its body, version and name included, and it occurred `at`.
pub fn published(
    routing: &str,
    kind: &str,
    id: &str,
    version: u64,
    event: Value,
    agent: &Agent,
    at: OffsetDateTime,
) -> Message {
    let payload = json!({
        "event": event,
        "aggregate": PublishedAggregate {
            id: id.to_owned(),
            kind: kind.to_owned(),
            version,
        },
        "agent": PublishedAgent::of(agent),
        "occurred_at": text::serialize(at),
    });

    Message::opening(
        RoutingKey::parse(routing).expect("a published event's routing key holds no wildcards"),
        payload,
        at,
    )
}
