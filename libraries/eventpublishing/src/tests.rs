use eventsourcing::{
    AccountId, Agent, AggregateContext, AggregateId, AggregateType, Event, EventMetadata,
    EventName, Recorded, Version,
};
use messaging::{Message, RoutingKey};
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
    Agent::Account(AccountId::from("author-7"))
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
            context: AggregateContext::default(),
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
    // expect: a key of the kind and the event name, an underscore separating too
    assert_eq!(routing_for(KIND, CAPTURED).to_string(), "idea.captured");
    assert_eq!(
        routing_for(KIND, SCENE_ATTACHED).to_string(),
        "idea.scene.attached",
        "an underscore becomes a separator, so a wildcard can select a subtree"
    );
}

#[test]
fn a_kind_can_be_subscribed_to_whole() {
    // expect: one pattern matches every event of a kind
    assert_eq!(everything_from(KIND), "idea.#");
}

#[test]
fn what_is_published_says_who_did_it() {
    // when: an author's capture is published
    let published = message_for(&recorded(a_capture(), 1, an_author()), body)
        .expect("a capture should be published");

    // then: it says the author did it
    assert_eq!(
        read(&published).agent,
        PublishedAgent {
            kind: "account".to_owned(),
            id: Some("author-7".to_owned()),
        },
        "a subscriber that cannot say who did it can never show a history"
    );
}

#[test]
fn a_local_agent_still_says_so() {
    // when: a capture done locally is published
    let published = message_for(&recorded(a_capture(), 1, Agent::Local), body)
        .expect("a capture should be published");

    // then: it says so
    assert_eq!(read(&published).agent.kind, "local");
    // and: names no one
    assert_eq!(read(&published).agent.id, None);
}

#[test]
fn what_is_published_names_the_aggregate_it_happened_to() {
    // when: a capture at version seven is published
    let published = message_for(&recorded(a_capture(), 7, an_author()), body)
        .expect("a capture should be published");

    // then: it names the aggregate, its kind and that version
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
    // when: a capture is published
    let published = message_for(&recorded(a_capture(), 1, an_author()), body)
        .expect("a capture should be published");

    // then: it carries the body the feature made of it, with its version
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
    // when: a capture is published
    let published = message_for(&recorded(a_capture(), 1, an_author()), body)
        .expect("a capture should be published");

    // then: its body sits flat beside its version
    assert_eq!(
        published.payload["event"],
        json!({ "version": 2, "name": "CAPTURED", "title": "The Loom" }),
        "a subscriber reads one flat event object, not a body nested inside a wrapper"
    );
}

#[test]
fn an_event_with_nothing_to_say_still_says_its_name() {
    // when: an event with nothing to say is published
    let published = message_for(&recorded(Happened::SceneAttached, 2, an_author()), body)
        .expect("an attachment should be published");

    // then: it is routed by its name
    assert_eq!(published.routing.to_string(), "idea.scene.attached");
    // and: its body still says its name
    assert_eq!(
        published.payload["event"],
        json!({ "version": 0, "name": "SCENE_ATTACHED" }),
        "a body with no fields must not compact the envelope around it"
    );
    // and: it reads back as that event
    assert_eq!(read(&published).event.body, Body::SceneAttached);
}

#[test]
fn what_is_published_says_when_it_happened_in_a_readable_way() {
    // when: a capture is published
    let published = message_for(&recorded(a_capture(), 1, an_author()), body)
        .expect("a capture should be published");

    // then: when it happened reads as a date and time
    assert_eq!(read(&published).occurred_at, "1970-01-01T00:16:40Z");
}

#[test]
fn a_snapshot_is_never_published() {
    // when: a snapshot is offered for publishing
    let nothing = message_for(&recorded(Happened::Snapshotted, 101, an_author()), body);

    // then: nothing is published
    assert!(
        nothing.is_none(),
        "collapsing the log is our own housekeeping and no subscriber's business"
    );
}

#[test]
fn an_event_the_feature_keeps_to_itself_is_not_published() {
    // given: a feature that keeps every event to itself
    fn nothing_ever(_event: &Happened) -> Option<Body> {
        None
    }

    // then: its capture is not published
    assert!(
        message_for(&recorded(a_capture(), 1, an_author()), nothing_ever).is_none(),
        "a feature must be able to keep an internal event off the wire"
    );
}

#[test]
fn a_message_that_is_not_a_published_event_is_refused() {
    // given: a message that is no published event
    let stray = Message::opening(
        routing_for(KIND, CAPTURED),
        json!({ "nothing": "useful" }),
        at(1_000),
    );

    // then: reading it as one is refused
    assert!(matches!(
        published_in::<Body>(&stray),
        Err(UnreadableMessage::NotAPublishedEvent(..))
    ));
}

#[test]
fn the_aggregate_a_message_is_about_is_read_as_its_own_kind_of_id() {
    // given: a published capture
    let message =
        message_for(&recorded(a_capture(), 1, an_author()), body).expect("a capture is published");

    // then: its aggregate reads as an id of text
    assert_eq!(
        aggregate_in::<String>(&message).expect("any text is an id of text"),
        "idea_1"
    );
    // and: not as a number
    assert!(matches!(
        aggregate_in::<u32>(&message),
        Err(UnreadableMessage::NotItsId(..))
    ));
}

#[test]
fn a_message_that_is_not_a_published_event_names_no_aggregate() {
    // given: a message that is no published event
    let stray = Message::opening(
        routing_for(KIND, CAPTURED),
        json!({ "nothing": "useful" }),
        at(1_000),
    );

    // then: it names no aggregate
    assert!(matches!(
        aggregate_in::<String>(&stray),
        Err(UnreadableMessage::NotAPublishedEvent(..))
    ));
}

#[test]
fn what_is_published_carries_the_context_its_aggregate_was_left_in() {
    // given: a capture of an aggregate left in a context
    let mut happened = recorded(a_capture(), 7, an_author());
    happened.metadata.context = AggregateContext::default().with("shelf", "shelf_1");

    // when: it is published
    let published = message_for(&happened, body).expect("a capture should be published");

    // then: the context is on the wire
    assert_eq!(published.payload["context"], json!({ "shelf": "shelf_1" }));
    // and: reads back
    assert_eq!(
        read(&published).context.get("shelf").map(String::as_str),
        Some("shelf_1")
    );
}

#[test]
fn an_aggregate_in_no_context_publishes_none() {
    // when: a capture of an aggregate in no context is published
    let published = message_for(&recorded(a_capture(), 7, an_author()), body)
        .expect("a capture should be published");

    // then: no context is on the wire
    assert!(
        published.payload.get("context").is_none(),
        "what belongs to the whole instance looks on the wire as it always did"
    );
    // and: an empty one reads back
    assert!(read(&published).context.is_empty());
}

#[test]
fn the_context_of_any_message_is_read_the_same_way_and_none_is_an_empty_one() {
    // given: a published event in a context
    let mut happened = recorded(a_capture(), 7, an_author());
    happened.metadata.context = AggregateContext::default().with("shelf", "shelf_1");
    let enveloped = message_for(&happened, body).expect("a capture should be published");
    // and: a plain message in another
    let flat = Message::opening(
        RoutingKey::parse("scene.deleted").expect("a plain key"),
        json!({ "scene": "scene_1", "context": { "shelf": "shelf_2" } }),
        at(1_000),
    );
    // and: a plain message in none
    let bare = Message::opening(
        RoutingKey::parse("scene.deleted").expect("a plain key"),
        json!({ "scene": "scene_1" }),
        at(1_000),
    );

    // then: each context reads the same way
    assert_eq!(context_in(&enveloped).get("shelf"), Some("shelf_1"));
    assert_eq!(context_in(&flat).get("shelf"), Some("shelf_2"));
    // and: none reads as an empty one
    assert!(context_in(&bare).is_empty());
}
