# eventsourcing

The event store and the service that drives aggregates over it. The outbox an append announces into is [its own library](../outbox).

**The model.** An `Aggregate` says how it is born (`begin`, `from_first`), what it will accept (`decide`), how it changes (`apply`), and how it collapses (`snapshot`). `EventSourcingService` replays a stream into state, runs a command against it, and appends what it decided under a `Version` guard, retrying a lost race a few times before giving up. `Standing<A>` is what a reader gets: the state and the version it stands at.

**Three backends, one conformance suite.** `InMemoryEventStore`, `PostgresEventStore` and `SqliteEventStore` are driven by the same case list through a `Workbench`, so "which store" is never a behavioural question. Storage shape is the feature's own: a `Codec<E>` converts its events to and from the JSON body, which is what keeps stored shapes out of the domain and makes upcasting a `Patch` applied on read rather than a rewrite in place.

**The dual write is closed.** `append` writes the event rows and, through the outbox's `enqueue`, a message for every event its `MessageMapping` publishes — in one transaction, so there is no window where an event is durable but unannounced, and a rolled-back append announces nothing. The outbox's table therefore has to be laid down beside the events: a feature's schema is `eventsourcing::postgres::migrations()` and `outbox::postgres::migrations()` together, each with its own ledger.

**`PublishingEventStore`** is the decorator for the in-memory case, which has no outbox to protect and publishes inline instead. [M11b](../../ROADMAP.md#milestone-11b--one-flow-in-every-mode) removes that difference.

**SQLite is local mode's store**, behind the `sqlite` feature, with its schema in `migrations/sqlite/` beside the PostgreSQL one. Two things differ from PostgreSQL on purpose. An append opens with `BEGIN IMMEDIATE`, taking the write lock before it reads the stream's head: a deferred transaction that read the head and then found another writer had committed cannot become a writer itself, and SQLite answers that with *busy* instead of waiting — so the losing writer would hear "the file was busy" where it should hear "your version is stale". And instants are stored as fixed-width UTC text, `2026-10-07T11:22:33.123456789Z`, readable in the file without the app and comparable as plain strings, the same format the outbox uses, from `outbox::sqlite::instant`.
