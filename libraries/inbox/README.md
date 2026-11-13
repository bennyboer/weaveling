# inbox

Records which messages a listener has already received. So a message that is delivered twice is handled only once. It is the counterpart of [the outbox](../outbox/README.md).

A message can reach a listener more than once:

- The relay may publish it again, after a crash between publishing it and marking it as published.
- A delivery is retried after a handler failed.

A listener's effects must not happen twice. So a `Delivery::Kept` listener is a **`KeptHandler`**. Its `take_in` receives a [`Transaction`](../transacting/README.md). It is called through **`Inboxed`**, which wraps it with its feature's `Transactions` and `Inbox`. A feature holds those two together as a **`Hearing`**. The `Hearing` turns each kept handler into a listener:

```rust
let hearing = Hearing::new(ports.transactions.clone(), ports.inbox.clone());

Arc::new(hearing.inboxed(NoteRoleChangedListener::new(notifications.clone())))
```

For each message, `Inboxed` does this:

1. It begins the transaction.
2. It asks the inbox whether this listener has received the message before (`heard_before`).
3. If not, it passes the transaction to the handler.
4. It finishes the transaction, once.

*Since 2026-11-04 this runs through `transacting::retrying!`.* So if the transaction itself lost a race, the whole step is retried at once, in a fresh transaction. This happens before the delivery would offer the message again.

The handler passes the transaction on to the core as `within`. So the record that the message was received commits together with the handler's effects, or not at all. The handler returns a `messaging::Failure`, as a plain handler does.

*A refused handler is not retried in process. Decided on 2026-11-02: the user chose this over wrapping the handling in `retrying!`.* If a handler's core lost a race, the handler is refused like any other. The delivery then offers the message again, after a growing pause, up to `messaging::ATTEMPTS` times, and the inbox check runs again with it. So a listener has one way of being retried, not two stacked on each other.

**`heard_before(transaction, listener, message)`** returns whether this listener has received this message before. If it has not, it records `(listener, message id)`. The record commits with the handler's effects, or rolls back with them. So a handler that failed receives its message as new when it is delivered again. A handler that succeeded sees it as already received. The record is per listener: when two listeners receive one message, each handles it for itself. If two handlers receive the same message at the same time, it is handled once. The second waits for the first one's transaction, and then finds the message already received.

**Records are kept for a while, then deleted.** A message is not delivered again once it is older than `messaging::RETRIABLE_FOR`. So the records do not need to be kept longer than that either. **`forget_heard_before(transaction, before, at_most)`** deletes the oldest records first, and no more than asked. **`HeardMessages::of(inbox, transactions)`** hands this to `chores::ForgetOldTombstones`. That chore keeps the records as long as a tombstone: `RETRIABLE_FOR` plus one day. It deletes in batches until a batch comes back with fewer than asked, each batch in its own transaction. A `Hearing` provides this chore, named after its feature:

```rust
hearing.forgetting("forget-old-notification-hearings", context.clock.clone())
```

**Where it lives.** It lives in the feature's own database, next to its other ports. It is only useful inside the handler's transaction. The deliveries are in the `messaging` database, so they cannot commit together with it.

- **`InMemoryInbox`** undoes what it recorded when the transaction is dropped.
- **`PostgresInbox`**, behind the `postgres` feature, and **`SqliteInbox`**, behind `sqlite`, create their table with `migrations()`. The table is tracked in its own ledger, `_sqlx_migrations_inbox`, as the outbox does it.

Each one takes the clock it uses to stamp `heard_at`. So deleting old records compares times from one and the same clock.

**Which listeners use it.** *Decided on 2026-11-02: the user chose every `Delivery::Kept` listener over only the listeners whose effects are not idempotent.* That includes a catalog that re-reads its aggregate, as much as a listener that runs a command. So no listener has to be judged safe without an inbox. The cost is one row and one transaction per received message.

Some listeners stay plain `messaging::Handler`s:

- A sweep. It spans many transactions on purpose. It is kept safe by its idempotent steps and by derived continuation ids.
- A `Delivery::Fleeting` listener. It never needs an inbox.

A listener may need to read what a racing writer committed, so that of two racing writers, one always sees the other. That is no reason to stay outside the inbox. The handler writes a follow-up message to itself in its transaction, through `Outbox::enqueue_within`. The follow-up is delivered once that transaction has committed. It then does the read, in a handling of its own.

A handler under `Inboxed` writes only through the transaction it receives. If it began a transaction of its own, then on SQLite it would wait for the single writer slot, which the inbox step is holding.

*The type system enforces this (2026-11-02).* A `messaging::Handler` must state why it has no inbox, through `WITHOUT_INBOX`: it is a sweep, or it is fleeting. Otherwise a kept listener is a `KeptHandler`, and it becomes a `Listener` only through `Inboxed`. A hand-written `Listener` can still go without an inbox, if that is intended.

**An effect outside the database.** *Added on 2026-11-04.* Some handlers do work outside the feature's database, such as sending a mail. Such a handler is an `EffectHandler`. It is called through `InboxedEffect`, created with `hearing.inboxed_effect(handler)`. It works in three steps:

1. It asks `was_heard` in a short transaction.
2. It does the work without holding any transaction.
3. It records the message as received, in another transaction.

So a slow or hanging mail server holds no writer slot, no connection and no lock. If the process crashes between the work and the record, the work is done again when the message arrives again. This is unavoidable, because nothing outside the database can be rolled back together with the inbox. If the work fails, nothing is recorded, so it is retried from scratch.

**Conformance.** `suite.rs` holds the cases that every backend passes:

- a message counts as received from the second time on,
- it never counts as received when the transaction was dropped,
- records are per listener and per message,
- the oldest records are deleted first, and no more than asked,
- the exact moment is kept,
- a dropped delete deletes nothing,
- two handlers at the same time handle a message once.

`kept_tests.rs` holds the cases for `Inboxed`:

- a message delivered twice is handled once,
- a message whose handling failed is handled again as new,
- a refusal is tried once and left for the delivery to offer again,
- the handler has the name and the subscriptions it declares.
