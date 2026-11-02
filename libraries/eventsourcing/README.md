# eventsourcing

The event store and the service that drives aggregates over it. The outbox an append announces into is [its own library](../outbox).

**The model.** An `Aggregate` says how it is born (`begin`, `from_first`), what it will accept (`decide`), how it changes (`apply`), and how it collapses (`snapshot`). `EventSourcingService` replays a stream into state, runs a command against it, and appends what it decided under a `Version` guard. A snapshot falls due every `snapshot_after()` events and is taken behind them in the command's transaction, so the version a command answers is the snapshot's, the one its caller aims at next; a snapshot that fails rolls the command back with it, so a command tried again is never appended twice. `Standing<A>` is what a reader gets: the state and the version it stands at.

The ideas drive their aggregate like this:

```rust
use std::sync::Arc;

use clock::{Clock, SystemClock};
use eventsourcing::{Agent, AggregateId, EventSourcingService, InMemoryEventStore, Standing};
use transacting::InMemoryTransactions;

let clock = Arc::new(SystemClock);
let ideas = EventSourcingService::<Idea>::new(
    Arc::new(InMemoryEventStore::new()),
    Arc::new(InMemoryTransactions::new()),
    clock.clone(),
);
let idea = AggregateId::from(&IdeaId::generate(clock.now()));

ideas.begin(None, &idea, IdeaCommand::Capture { project, title }, &Agent::Local).await?;
ideas.execute(None, &idea, IdeaCommand::Discard, &Agent::Local).await?;

let Standing { state, version } = ideas.latest(&idea).await?;
```

**Writes go through a transaction.** `append`, `prune_through` and `forget` take a [`transacting::Transaction`](../transacting/README.md), so a feature's other ports can join the same one. *A command reads through it too (2026-11-02, the integrity audit):* `read_from_within` and `latest_snapshot_within` read the stream and its newest snapshot through the transaction they are given, as the service does for `execute`, `execute_at`, `compact` and the snapshot it makes due; so a command needs no connection beside the one its transaction holds, where five at once used to wait on PostgreSQL's pool of five for a sixth, and a second command on an aggregate in the same transaction reads what the first appended, where it used to be outdated on every try on PostgreSQL and SQLite and only memory saw it. Reads outside a command, `latest` and `as_of` for queries, take none and stay on the pool. The service opens no transaction of its own and holds no `Transactions`: `begin`, `execute`, `execute_at` and `compact` take the caller's `&mut Transaction` and append in it, the snapshot with them, leaving the commit and any retry after a lost race to the core that owns it, who alone can try the whole unit again, with [`transacting::retrying!`](../transacting/README.md) and `tries_for(expected)`; `ServiceError` is a `LostRace` when the store answers `Outdated`, and takes a `Failure` as `ServiceError::Transaction`, so it can be retried as it is. A command within a transaction reads the stream as committed, so it does not see what that same transaction appended before it. `compact` takes its snapshot and prunes in the one transaction it is handed. In memory, a write registers how to undo it, and its messages are enqueued only once the transaction commits.

**A stream is collapsed or forgotten.** `prune_through` deletes the events a snapshot replaced and keeps the snapshot, so the stream keeps its history, however little, and cannot be begun again. `forget` deletes a stream whole, snapshot and all, for what no history is kept of (a notice put away long since); a stream forgotten could be begun again from version zero, so whoever forgets one makes sure nothing will. Both in memory, on PostgreSQL and on SQLite, one conformance suite for all three, which also proves that an append, a prune or a forget whose transaction is dropped leaves the stream and the outbox as they were.

**Every event carries its aggregate's context.** An `Aggregate` may say what it belongs to as an `AggregateContext`, a small map of names to values, empty unless it says otherwise; the service stamps the context of the state after a command on every event the command wrote, and on a snapshot, and the stores keep it beside the event, as a JSON column. What the names mean is the features' business, not this library's: it knows no project, only that an aggregate may be in one, a name and a value. Messages carry it on, so a listener can tell what an event belonged to without asking anyone.

**A store that refuses every append**, `RefusesAppending`, behind the `testing` feature, reads through to the store it wraps and answers every write with a backend failure, for a feature's tests to prove that a use case whose append fails leaves nothing else behind in its transaction.

**Three backends, one conformance suite.** `InMemoryEventStore`, `PostgresEventStore` and `SqliteEventStore` are driven by the same case list through a `Workbench`, so "which store" is never a behavioural question. Storage shape is the feature's own: a `Codec<E>` converts its events to and from the JSON body, which is what keeps stored shapes out of the domain and makes upcasting a `Patch` applied on read rather than a rewrite in place.

**The dual write is closed.** `append` writes the event rows and, through the outbox's `enqueue`, a message for every event its `MessageMapping` publishes — in one transaction, so there is no window where an event is durable but unannounced, and a rolled-back append announces nothing. The outbox's table therefore has to be laid down beside the events: a feature's schema is `eventsourcing::postgres::migrations()` and `outbox::postgres::migrations()` together, each with its own ledger.

**SQLite is local mode's store**, behind the `sqlite` feature, with its schema in `migrations/sqlite/` beside the PostgreSQL one. Two things differ from PostgreSQL on purpose. An append's transaction opens with `BEGIN IMMEDIATE`, as every `SqliteTransactions` does, taking the write lock before it reads the stream's head: a deferred transaction that read the head and then found another writer had committed cannot become a writer itself, and SQLite answers that with *busy* instead of waiting — so the losing writer would hear "the file was busy" where it should hear "your version is stale". And instants are stored as fixed-width UTC text, `2026-10-07T11:22:33.123456789Z`, readable in the file without the app and comparable as plain strings, the same format the outbox uses, from `outbox::sqlite::instant`.
