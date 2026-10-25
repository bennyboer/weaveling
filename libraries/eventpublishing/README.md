# eventpublishing

Turning a recorded event into a message. It is the third thing between [`eventsourcing`](../eventsourcing) and [`messaging`](../messaging), and it depends on both so that neither depends on the other.

The envelope is fixed here so every feature's events look alike on the wire: a `PublishedEvent` carries the aggregate it happened to, the agent who caused it, when it occurred, and the body the feature supplies. The routing key is derived — `routing_for(KIND, name)` gives `idea.captured` — and `everything_from(KIND)` gives the `idea.#` a listener binds to, so a publisher and its subscribers cannot drift apart on a string.

**The envelope carries the aggregate's context** beside the aggregate, the agent and the moment, `"context": { "project": "…" }`, and leaves it out when empty, so an instance-wide aggregate's messages look as they always did.

**What a feature still owns** is the mapping from its own event to a DTO, and it returns `Option`: a snapshot is housekeeping and publishes nothing. That mapping lives in each feature's `adapters/messaging` as its `message_for`, which the wiring hands to the event store.

`published_in(message)` reads one back, which is how a projector recovers the aggregate id from a message it was handed.
