# eventpublishing

Turns a recorded event into a message. It sits between [`eventsourcing`](../eventsourcing) and [`messaging`](../messaging) as a third library. It depends on both, so that neither of them depends on the other.

The envelope is defined here, so every feature's events look the same on the wire. A `PublishedEvent` carries:

- the aggregate the event happened to,
- the agent who caused it,
- when it occurred,
- the body that the feature supplies.

The routing key is derived. `routing_for(KIND, name)` returns `idea.captured`. `everything_from(KIND)` returns `idea.#`, which a listener binds to. So a publisher and its subscribers cannot end up with different strings.

This is how the ideas feature publishes its events:

```rust
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
```

**The envelope also carries the aggregate's context,** next to the aggregate, the agent and the moment: `"context": { "project": "…" }`. When the context is empty, it is left out. So the messages of an instance-wide aggregate look the same as they always did. `context_in(message)` reads the context from any message. That can be an envelope, or a flat message that a feature built itself, such as the scenes' messages. If a message has no context, it returns an empty one.

**What a feature still owns** is the mapping from its own event to a DTO. The mapping returns an `Option`, because a snapshot is housekeeping and publishes nothing. The mapping lives in each feature's `adapters/messaging` as its `message_for`. The wiring hands it to the event store.

`published_in(message)` reads a message back. A projector uses it to get the aggregate id from a message it received. A test can give a listener such a message with **`published(routing, kind, id, version, event, agent, at)`**, behind the `testing` feature. That way the test does not have to write the envelope by hand.
