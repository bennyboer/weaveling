use eventsourcing::{
    Agent, AgentId, AggregateId, AggregateType, Event, EventMetadata, EventName, Recorded, Version,
};
use messaging::Message;
use serde::{Deserialize, Serialize};
use serde_json::json;
use time::{Duration, OffsetDateTime};

use super::*;

const KIND: AggregateType = AggregateType::of("idea");
const CAPTURED: EventName = EventName::of("CAPTURED");
const SCENE_ATTACHED: EventName = EventName::of("SCENE_ATTACHED");
const SNAPSHOTTED: EventName = EventName::of("SNAPSHOTTED");

#[derive(Debug, Clone, PartialEq, Eq)]
enum Happened {
    Captured { title: String },
    SceneAttached,
    Snapshotted,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, Serialize)]
#[serde(tag = "name")]
enum Body {
    #[serde(rename = "CAPTURED")]
    Captured { title: String },
    #[serde(rename = "SCENE_ATTACHED")]
    SceneAttached,
}

impl Event for Happened {
    fn name(&self) -> EventName {
        match self {
            Self::Captured { .. } => CAPTURED,
            Self::SceneAttached => SCENE_ATTACHED,
            Self::Snapshotted => SNAPSHOTTED,
        }
    }

    fn version(&self) -> Version {
        match self {
            Self::Captured { .. } => Version::of(2),
            _ => Version::ZERO,
        }
    }

    fn is_snapshot(&self) -> bool {
        matches!(self, Self::Snapshotted)
    }
}

fn body(event: &Happened) -> Option<Body> {
    match event {
        Happened::Captured { title } => Some(Body::Captured {
            title: title.clone(),
        }),
        Happened::SceneAttached => Some(Body::SceneAttached),
        Happened::Snapshotted => None,
    }
}

fn at(seconds: i64) -> OffsetDateTime {
    OffsetDateTime::UNIX_EPOCH + Duration::seconds(seconds)
}

fn an_author() -> Agent {
    Agent::User(AgentId::from("author-7"))
}

fn recorded(event: Happened, version: u64, agent: Agent) -> Recorded<Happened> {
    Recorded {
        metadata: EventMetadata {
            aggregate: AggregateId::from("idea_1"),
            kind: KIND,
            version: Version::of(version),
            agent,
            occurred_at: at(1_000),
            is_snapshot: event.is_snapshot(),
        },
        event,
    }
}

fn a_capture() -> Happened {
    Happened::Captured {
        title: "The Loom".to_owned(),
    }
}

fn read(message: &Message) -> PublishedEvent<Body> {
    published_in(message).expect("what we just wrote must be readable")
}

#[test]
fn a_routing_key_is_the_kind_and_the_event_name() {
    assert_eq!(routing_for(KIND, CAPTURED).to_string(), "idea.captured");
    assert_eq!(
        routing_for(KIND, SCENE_ATTACHED).to_string(),
        "idea.scene.attached",
        "an underscore becomes a separator, so a wildcard can select a subtree"
    );
}

#[test]
fn a_kind_can_be_subscribed_to_whole() {
    assert_eq!(everything_from(KIND), "idea.#");
}

#[test]
fn what_is_published_says_who_did_it() {
    let published = message_for(&recorded(a_capture(), 1, an_author()), body)
        .expect("a capture should be published");

    assert_eq!(
        read(&published).agent,
        PublishedAgent {
            kind: "user".to_owned(),
            id: Some("author-7".to_owned()),
        },
        "a subscriber that cannot say who did it can never show a history"
    );
}

#[test]
fn an_anonymous_agent_still_says_so() {
    let published = message_for(&recorded(a_capture(), 1, Agent::Anonymous), body)
        .expect("a capture should be published");

    assert_eq!(read(&published).agent.kind, "anonymous");
    assert_eq!(read(&published).agent.id, None);
}

#[test]
fn what_is_published_names_the_aggregate_it_happened_to() {
    let published = message_for(&recorded(a_capture(), 7, an_author()), body)
        .expect("a capture should be published");

    assert_eq!(
        read(&published).aggregate,
        PublishedAggregate {
            id: "idea_1".to_owned(),
            kind: "idea".to_owned(),
            version: 7,
        },
        "a projection guarding against stale news needs the version, not just the id"
    );
}

#[test]
fn what_is_published_carries_the_body_the_feature_supplied() {
    let published = message_for(&recorded(a_capture(), 1, an_author()), body)
        .expect("a capture should be published");

    assert_eq!(
        read(&published).event,
        PublishedBody {
            version: 2,
            body: Body::Captured {
                title: "The Loom".to_owned(),
            },
        },
        "the library owns the envelope and the feature owns what the event body"
    );
}

#[test]
fn the_body_sits_beside_its_version_on_the_wire() {
    let published = message_for(&recorded(a_capture(), 1, an_author()), body)
        .expect("a capture should be published");

    assert_eq!(
        published.payload["event"],
        json!({ "version": 2, "name": "CAPTURED", "title": "The Loom" }),
        "a subscriber reads one flat event object, not a body nested inside a wrapper"
    );
}

#[test]
fn an_event_with_nothing_to_say_still_says_its_name() {
    let published = message_for(&recorded(Happened::SceneAttached, 2, an_author()), body)
        .expect("an attachment should be published");

    assert_eq!(published.routing.to_string(), "idea.scene.attached");
    assert_eq!(
        published.payload["event"],
        json!({ "version": 0, "name": "SCENE_ATTACHED" }),
        "a body with no fields must not compact the envelope around it"
    );
    assert_eq!(read(&published).event.body, Body::SceneAttached);
}

#[test]
fn what_is_published_says_when_it_happened_in_a_readable_way() {
    let published = message_for(&recorded(a_capture(), 1, an_author()), body)
        .expect("a capture should be published");

    assert_eq!(read(&published).occurred_at, "1970-01-01T00:16:40Z");
}

#[test]
fn a_snapshot_is_never_published() {
    let nothing = message_for(&recorded(Happened::Snapshotted, 101, an_author()), body);

    assert!(
        nothing.is_none(),
        "collapsing the log is our own housekeeping and no subscriber's business"
    );
}

#[test]
fn an_event_the_feature_keeps_to_itself_is_not_published() {
    fn nothing_ever(_event: &Happened) -> Option<Body> {
        None
    }

    assert!(
        message_for(&recorded(a_capture(), 1, an_author()), nothing_ever).is_none(),
        "a feature must be able to keep an internal event off the wire"
    );
}

#[test]
fn a_message_that_is_not_a_published_event_is_refused() {
    let stray = Message::opening(
        routing_for(KIND, CAPTURED),
        json!({ "nothing": "useful" }),
        at(1_000),
    );

    assert!(matches!(
        published_in::<Body>(&stray),
        Err(UnreadableMessage::NotAPublishedEvent(..))
    ));
}

#[test]
fn the_aggregate_a_message_is_about_is_read_as_its_own_kind_of_id() {
    let message =
        message_for(&recorded(a_capture(), 1, an_author()), body).expect("a capture is published");

    assert_eq!(
        aggregate_in::<String>(&message).expect("any text is an id of text"),
        "idea_1"
    );
    assert!(matches!(
        aggregate_in::<u32>(&message),
        Err(UnreadableMessage::NotItsId(..))
    ));
}

#[test]
fn a_message_that_is_not_a_published_event_names_no_aggregate() {
    let stray = Message::opening(
        routing_for(KIND, CAPTURED),
        json!({ "nothing": "useful" }),
        at(1_000),
    );

    assert!(matches!(
        aggregate_in::<String>(&stray),
        Err(UnreadableMessage::NotAPublishedEvent(..))
    ));
}
