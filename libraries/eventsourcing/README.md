# eventsourcing

The event store, the service that drives aggregates over it, and — on PostgreSQL — the transactional outbox and its relay.

**The model.** An `Aggregate` says how it is born (`begin`, `from_first`), what it will accept (`decide`), how it changes (`apply`), and how it collapses (`snapshot`). `EventSourcingService` replays a stream into state, runs a command against it, and appends what it decided under a `Version` guard, retrying a lost race a few times before giving up. `Standing<A>` is what a reader gets: the state and the version it stands at.

**Two backends, one conformance suite.** `InMemoryEventStore` and `PostgresEventStore` are driven by the same case list through a `Workbench`, so "which store" is never a behavioural question. Storage shape is the feature's own: a `Codec<E>` converts its events to and from the `jsonb` body, which is what keeps stored shapes out of the domain and makes upcasting a `Patch` applied on read rather than a rewrite in place.

**The dual write is closed.** `append` writes the event rows and the outbox rows in one transaction and notifies on that same transaction, so there is no window where an event is durable but unannounced — and a rolled-back append announces nothing. `RelayTask` runs two loops per outbox: one delivering, woken by the notification and polling only as a backstop; one sweeping published entries past `KEPT_FOR`. `claimed_until` stops a crashed relay stranding a message, and `FOR UPDATE SKIP LOCKED` is what lets two relays run at all.

**`PublishingEventStore`** is the decorator for the in-memory case, which has no outbox to protect and publishes inline instead. [M11b](../../ROADMAP.md#milestone-11b--one-flow-in-every-mode) removes that difference.
