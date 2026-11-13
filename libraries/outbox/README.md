# outbox

The transactional outbox and its relay. They make sure that a change that commits is always announced, and that a change that rolls back is never announced.

**Enqueue inside the writer's transaction.** There are several ways to put a message into the outbox:

- `postgres::enqueue(transaction, origin, message)` and `sqlite::enqueue(…)` put a message into the outbox table inside *any* writer's [`transacting::Transaction`](../transacting/README.md).
- `InMemoryOutbox::enqueue_after_commit(transaction, message)` holds the message back until that transaction commits.
- `Outbox::enqueue_within(transaction, origin, message)` does whatever its outbox's backend does. It is for a writer that does not know its backend, such as a feature's core that sends a follow-up to itself.

The writers are the event store's `append`, or a feature that is not event-sourced at all. An example of the latter is scenes, whose prose is a CRDT.

`Origin` records what the message is about (`aggregate`, `kind`, `version`). This is for whoever reads the table. The outbox itself never reads it back. The event store passes its event's version. A writer without versions, such as the scenes or a follow-up, uses `Origin::unversioned(aggregate, kind)`. Its version is none, which is a null in the table. *Until 2026-11-02 it was `0`. The user asked why a follow-up said version 0, because that reads like a real version.*

A listener that sends a follow-up to its own feature, in its inbox transaction, does it with one call: `outbox.enqueue_follow_up_within(transaction, origin, heard, routing, &payload, now)`. This builds the answer to the received message with `Message::follow_up`, and enqueues it.

The relay wakes up when the write commits, and not before. PostgreSQL's `NOTIFY` is sent in the same transaction. SQLite and memory ring a bell once the transaction is committed, through `Transaction::after_commit`. A transaction that is dropped unfinished takes its messages with it and wakes nobody.

The scenes feature announces a change in the transaction that stores it, and a relay sends it on:

```rust
use outbox::{Cadence, Origin, RelayTask};

let mut transaction = transactions.begin().await?;
store_the_change(&mut transaction).await?;

let scene = change.scene().to_string();
let origin = Origin::unversioned(&scene, "scene");
outbox::postgres::enqueue(&mut transaction, origin, &mapping.message(&change)).await?;

transaction.finish().await?;

let relay = RelayTask::started(scenes_outbox, Cadence::default());
relay.stop().await;
```

**The relay.** `RelayTask` runs two loops per `Outbox`:

- One delivers. It is woken by the notification, and polls only as a backstop.
- One sweeps away published entries older than `KEPT_FOR`.

A claimed entry is left alone for `CLAIM_FOR`. This stops a crashed relay from stranding a message. On PostgreSQL, `FOR UPDATE SKIP LOCKED` is what makes it possible to run two relays at all. A retry publishes the message with the id it was written with. So the receiving end can recognise a redelivery.

**An aggregate's published messages can be deleted sooner.** `outbox::postgres::forget_published` and `outbox::sqlite::forget_published` delete the published entries of one aggregate, in the caller's transaction. `InMemoryOutbox::forget_published_after_commit` does the same once its transaction commits. Entries that are still waiting are left to go out. The event stores call these through `EventStore::forget_published`. Two callers use that after compacting: the sweep of a deleted account, and an invitation that is retired. So a name or address that their messages carried is not kept for another `KEPT_FOR`. The in-memory outbox keeps each entry's origin for this, through `enqueue_after_commit_from`.

**This holds for the relay's retries, not for a listener's.** Some listeners answer a message with another message. The ideas and scenes sweeps do this with `MORE_TO_SWEEP`. Such a listener creates a fresh id through `Message::answering` each time it runs. When the broker redelivers the message it is answering, the second answer is a new message. No deduplication by id can tell it apart from the first answer. Such an answer needs an id derived from the message it answers. See [TODO](../../TODO.md).

**There are three outboxes.** `InMemoryOutbox` is for the in-memory mode. `PostgresOutbox` is behind the `postgres` feature. `SqliteOutbox` is behind `sqlite`, for local mode. Each creates its own table with `migrations()`. The table is tracked in its own ledger, `_sqlx_migrations_outbox`. So the outbox sits next to whatever else owns the database, and neither knows about the other.

**Why it is not part of `eventsourcing`.** It started there, because the event store was its first writer. But a writer that is not event-sourced should not need an event store to announce its changes. And its database should not get an `events` table it never uses. So the dependency points one way: `eventsourcing` enqueues through the outbox, and the outbox knows nothing about events.

**Messages go out in the order they were written.** A claim comes back from `UPDATE … RETURNING` in whatever order the database chooses. So every outbox sorts what it claimed by entry before publishing. If a `Detached` overtook the `Attached` it undoes, a read model would stay wrong for good.

The in-memory outbox numbers its entries with a counter, never by how many entries it holds. So an entry written after the sweep removed published entries never gets a number that an older entry still has (*fixed on 2026-11-02*). And marking that entry as published marks the right one.

**The SQLite outbox rings an in-process bell.** SQLite has no `NOTIFY`. Its commit hook runs before the commit is visible. So `enqueue` rings a bell in the process once the writer's transaction has committed. `Cadence::deliver_every` is the backstop, for a writer in another process. See [the decision](../../ROADMAP.md#sqlite-backs-local-mode--decided). Claims expire by comparing instants that are stored as fixed-width UTC text. Such text sorts as strings in the same order as in time.
