# outbox

The transactional outbox and its relay: how a change that commits is always announced, and one that rolls back never is.

**Enqueue inside the writer's transaction.** `postgres::enqueue(transaction, origin, message)` and `sqlite::enqueue(…)` put a message into the outbox table inside *any* writer's [`transacting::Transaction`](../transacting/README.md), and `InMemoryOutbox::enqueue_after_commit(transaction, message)` holds it back until that transaction commits; `Outbox::enqueue_within(transaction, origin, message)` does whichever its outbox's backend does, for a writer that does not know its backend, as a feature's core asking a follow-up of itself — the event store's `append`, or a feature that is not event-sourced at all, like scenes, whose prose is a CRDT. `Origin` records what the message is about (`aggregate`, `kind`, `version`), for whoever reads the table, the outbox itself never reading it back; the event store passes its event's version, and a writer with no versions, as the scenes or a follow-up, `Origin::unversioned(aggregate, kind)`, its version none, a null in the table, *not the `0` it was until 2026-11-02, the user asking why a follow-up said version 0, which reads as a version.* A listener asking a follow-up of its own feature in its hearing's transaction says it in one call, `outbox.enqueue_follow_up_within(transaction, origin, heard, routing, &payload, now)`, which answers the message heard with `Message::follow_up` and enqueues the answer. The relay wakes when the write commits and not before: PostgreSQL's `NOTIFY` is sent in the same transaction, and SQLite and memory ring once it is committed, through `Transaction::after_commit`. A transaction dropped unfinished takes its messages with it and wakes nobody.

The scenes announce a change in the transaction that stores it, and a relay sends it on:

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

**The relay.** `RelayTask` runs two loops per `Outbox`: one delivering, woken by the notification and polling only as a backstop; one sweeping published entries past `KEPT_FOR`. A claimed entry is left alone for `CLAIM_FOR`, which is what stops a crashed relay stranding a message, and on PostgreSQL `FOR UPDATE SKIP LOCKED` is what lets two relays run at all. A retry publishes the message with the id it was written with, so the far end can recognise a redelivery.

**That holds for the relay's retries, not a listener's.** A listener that answers a message with another one, as the ideas and scenes sweeps do with `MORE_TO_SWEEP`, mints a fresh id through `Message::answering` each time it runs. When the broker redelivers what it is answering, the second answer is a new message, and no dedup on its id can tell it from the first. Such an answer needs an id derived from what it answers; see [TODO](../../TODO.md).

**Three outboxes.** `InMemoryOutbox` for the in-memory mode, `PostgresOutbox` behind the `postgres` feature, and `SqliteOutbox` behind `sqlite` for local mode. Each lays its own table down with `migrations()`, tracked in its own ledger, `_sqlx_migrations_outbox`, so it sits beside whatever else owns the database without either knowing about the other.

**Why it is not part of `eventsourcing`.** It began there, because the event store was its first writer. But a writer that is not event-sourced should not need an event store to announce its changes, and its database should not grow an `events` table it never uses. So the dependency points one way: `eventsourcing` enqueues through the outbox, and the outbox knows nothing about events.

**Messages go out in the order they were written.** A claim comes back from `UPDATE … RETURNING` in whatever order the database likes, so every outbox sorts what it claimed by entry before publishing — a `Detached` overtaking the `Attached` it undoes would leave a read model wrong for good. The in-memory outbox numbers its entries by a counter, never by how many it holds, so an entry written after the sweep took published ones away gets no number an older one still has (*fixed 2026-11-02*), and marking it published marks it.

**The SQLite outbox rings an in-process bell.** SQLite has no `NOTIFY`, and its commit hook runs before the commit is visible, so `enqueue` rings a bell in the process once the writer's transaction has committed, and `Cadence::deliver_every` is the backstop for a writer in another process; see [the decision](../../ROADMAP.md#sqlite-backs-local-mode--decided). Claims expire by comparing instants stored as fixed-width UTC text, which sort as strings the way they sort in time.
