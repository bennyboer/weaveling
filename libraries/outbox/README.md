# outbox

The transactional outbox and its relay: how a change that commits is always announced, and one that rolls back never is.

**Enqueue inside the writer's transaction.** `postgres::enqueue(transaction, origin, message)` and `sqlite::enqueue(…)` put a message into the outbox table inside *any* writer's transaction — the event store's `append`, or a feature that is not event-sourced at all, like passages, whose prose is a CRDT. `Origin` records what the message is about (`aggregate`, `kind`, `version`); a writer with no versions passes `0`. On PostgreSQL the same transaction notifies, so the relay wakes when the write commits.

**The relay.** `RelayTask` runs two loops per `Outbox`: one delivering, woken by the notification and polling only as a backstop; one sweeping published entries past `KEPT_FOR`. A claimed entry is left alone for `CLAIM_FOR`, which is what stops a crashed relay stranding a message, and on PostgreSQL `FOR UPDATE SKIP LOCKED` is what lets two relays run at all. A retry publishes the message with the id it was written with, so the far end can recognise a redelivery.

**Three outboxes.** `InMemoryOutbox` for the in-memory mode, `PostgresOutbox` behind the `postgres` feature, and `SqliteOutbox` behind `sqlite` for local mode. Each lays its own table down with `migrations()`, tracked in its own ledger, `_sqlx_migrations_outbox`, so it sits beside whatever else owns the database without either knowing about the other.

**Why it is not part of `eventsourcing`.** It began there, because the event store was its first writer. But a writer that is not event-sourced should not need an event store to announce its changes, and its database should not grow an `events` table it never uses. So the dependency points one way: `eventsourcing` enqueues through the outbox, and the outbox knows nothing about events.

**Messages go out in the order they were written.** A claim comes back from `UPDATE … RETURNING` in whatever order the database likes, so every outbox sorts what it claimed by entry before publishing — a `Detached` overtaking the `Attached` it undoes would leave a read model wrong for good.

**The SQLite outbox is polled, not notified.** It hands the relay notifications that never fire, so delivery rides on `Cadence::deliver_every` alone — short in local mode. `enqueue` is a free function inside the writer's transaction with nothing to wake, and SQLite's commit hook runs before the commit is visible; see [the decision](../../ROADMAP.md#sqlite-backs-local-mode--decided). Claims expire by comparing instants stored as fixed-width UTC text, which sort as strings the way they sort in time.
