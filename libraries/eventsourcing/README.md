# eventsourcing

The event store, and the service that runs aggregates on top of it. The outbox that an append announces its messages into is [its own library](../outbox).

**The model.** An `Aggregate` says:

- how it is created (`begin`, `from_first`),
- which commands it accepts (`decide`),
- how it changes (`apply`),
- how it is collapsed into a snapshot (`snapshot`).

`EventSourcingService` replays a stream into state, runs a command against that state, and appends the resulting events under a `Version` guard. A snapshot is due every `snapshot_after()` events. It is taken after those events, in the command's transaction. So the version a command returns is the snapshot's version, which is the one the caller targets next. If the snapshot fails, the command is rolled back with it. So a retried command is never appended twice. `Standing<A>` is what a reader gets: the state and the version it is at.

The ideas feature runs its aggregate like this:

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

**Writes go through a transaction.** `append`, `prune_through` and `forget` take a [`transacting::Transaction`](../transacting/README.md). So a feature's other ports can join the same transaction.

*A command also reads through the transaction (decided on 2026-11-02, in the integrity audit).* `read_from_within` and `latest_snapshot_within` read the stream and its newest snapshot through the transaction they are given. The service does this for `execute`, `execute_at`, `compact` and for the snapshot that falls due. This has two effects:

- A command needs no connection besides the one its transaction holds. Before, five commands at once each waited for a sixth connection from PostgreSQL's pool of five.
- A second command on the same aggregate in the same transaction reads what the first command appended. Before, it was outdated on every try on PostgreSQL and SQLite, and only the in-memory store saw the appended events.

Reads outside a command, `latest` and `as_of` for queries, take no transaction. They stay on the pool.

The service opens no transaction of its own and holds no `Transactions`. `begin`, `execute`, `execute_at` and `compact` take the caller's `&mut Transaction` and append in it, together with the snapshot. The commit, and any retry after a lost race, are left to the core that owns the transaction. Only the core can retry the whole unit of work. It does so with [`transacting::retrying!`](../transacting/README.md) and `tries_for(expected)`. `ServiceError` is a `LostRace` when the store returns `Outdated`. A `Failure` becomes `ServiceError::Transaction`, so it can be retried unchanged.

A command inside a transaction reads the stream as committed. So it does not see what the same transaction appended before it. `compact` takes its snapshot and prunes in the one transaction it is given. In memory, a write registers how to undo itself, and its messages are enqueued only once the transaction commits.

**A stream is collapsed or forgotten.**

- `prune_through` deletes the events that a snapshot replaced, and keeps the snapshot. So the stream keeps its history, however small, and cannot be started again.
- `forget` deletes a whole stream, including its snapshot. It is for things whose history is not kept, such as a notice that was dismissed long ago. A forgotten stream could be started again from version zero. So whoever forgets a stream must make sure nothing will start it again.
- `forget_published` deletes, in the same transaction, the stream's messages that the outbox has already published. It keeps the messages that are still waiting. So a stream that was compacted to keep nothing personal does not leave personal data in the outbox either (`EventSourcingService::forget_published`).

All of these exist in memory, on PostgreSQL and on SQLite. One conformance suite covers all three. The suite also proves that an append, a prune or a forget whose transaction is dropped leaves the stream and the outbox unchanged.

**Every event carries its aggregate's context.** An `Aggregate` may say what it belongs to, as an `AggregateContext`. That is a small map of names to values, and it is empty unless the aggregate says otherwise. After a command, the service stamps the context of the new state on every event the command wrote, and on a snapshot. The stores keep it next to the event, in a JSON column. What the names mean is the features' business, not this library's. The library knows no project. It only knows that an aggregate may belong to something, given as a name and a value. Messages carry the context on. So a listener can tell what an event belonged to without asking anyone.

**A store that refuses every append**, `RefusesAppending`, is available behind the `testing` feature. It reads through to the store it wraps, and answers every write with a backend failure. A feature's tests use it to prove that when a use case's append fails, nothing else is left behind in its transaction.

**`an_author()`**, behind the same feature, is the single account a test acts as, wherever it does not matter who acted.

**Three backends, one conformance suite.** `InMemoryEventStore`, `PostgresEventStore` and `SqliteEventStore` all run the same list of cases through a `Workbench`. So the choice of store never changes behaviour. The storage shape belongs to the feature. A `Codec<E>` converts the feature's events to and from the JSON body. This keeps stored shapes out of the domain. It also makes upcasting a `Patch` that is applied when reading, instead of a rewrite in place.

**The dual write is closed.** `append` writes the event rows. Through the outbox's `enqueue`, it also writes a message for every event that its `MessageMapping` publishes. Both happen in one transaction. So there is no moment where an event is stored but not announced. And a rolled-back append announces nothing. Because of this, the outbox's table must be created next to the events. A feature's schema is `eventsourcing::postgres::migrations()` plus `outbox::postgres::migrations()`, each with its own ledger.

**SQLite is the store for local mode.** It is behind the `sqlite` feature. Its schema is in `migrations/sqlite/`, next to the PostgreSQL one. Two things differ from PostgreSQL on purpose:

- An append's transaction opens with `BEGIN IMMEDIATE`, as every `SqliteTransactions` does. It takes the write lock before it reads the stream's head. The reason: a deferred transaction that has read the head and then finds that another writer committed cannot become a writer itself. SQLite answers that case with *busy* instead of waiting. So the losing writer would get "the file was busy", when it should get "your version is stale".
- Instants are stored as fixed-width UTC text, such as `2026-10-07T11:22:33.123456789Z`. This text is readable in the file without the app, and it can be compared as plain strings. It is the same format the outbox uses, from `outbox::sqlite::instant`.
