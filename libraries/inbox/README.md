# inbox

What a listener has heard already, so a message handed over twice is taken in once. The dual of [the outbox](../outbox/README.md).

A message can arrive at a listener more than once: the relay may publish it again after a crash between publishing and marking it published, and a delivery is tried again after a handler failed. A listener's effects must not happen twice, so a `Delivery::Kept` listener is a **`KeptHandler`**, whose `take_in` is handed a [`Transaction`](../transacting/README.md), and is heard through **`Inboxed`**, which wraps it with its feature's `Transactions` and `Inbox`:

```rust
Arc::new(Inboxed::new(
    NoteRoleChangedListener::new(notifications.clone()),
    ports.transactions.clone(),
    ports.inbox.clone(),
))
```

For each message `Inboxed` begins the transaction, asks the inbox whether this listener has `heard_before` the message, hands the transaction to the handler if not and finishes it, all through `transacting::retrying!`: a handler whose core lost a race is tried again whole, in a fresh transaction, the hearing with it, up to five times. The handler passes the transaction on to core as `within`, so the record of having heard the message commits with its effects or with none of them. It answers **`Refused`**, which any error turns into; one from core is turned with `Refused::of`, which keeps whether it was a lost race, so `Inboxed` knows to try again.

**`heard_before(transaction, listener, message)`** answers whether this listener had heard this message before, and records `(listener, message id)` if it had not. The record commits with the handler's effects or rolls back with them, so a handler that failed hears its message anew when it is redelivered, and one that succeeded hears it as heard before. It is per listener: two listeners hear one message each for themselves. Two handlers hearing one message at once hear it once; the second waits for the first's transaction and then finds it heard.

**Kept for a while, then forgotten.** A message is not handed over again once it is older than `messaging::RETRIABLE_FOR`, so what was heard need not be kept longer either. **`forget_heard_before(transaction, before, at_most)`** forgets the oldest first, no more than asked. **`HeardMessages::of(inbox, transactions)`** hands that to `chores::ForgetOldTombstones`, which keeps what was heard as long as a tombstone, `RETRIABLE_FOR` and a day, and forgets in batches until one comes back short, each in a transaction of its own:

```rust
ForgetOldTombstones::new(
    "forget-old-notification-hearings",
    Arc::new(HeardMessages::of(ports.inbox.clone(), ports.transactions.clone())),
    context.clock.clone(),
)
```

**Where it lives.** In the feature's own database, beside its other ports, since it is only worth something inside the handler's transaction; the deliveries, in the `messaging` database, cannot commit with it. **`InMemoryInbox`** undoes what it recorded when the transaction is dropped, **`PostgresInbox`** behind the `postgres` feature and **`SqliteInbox`** behind `sqlite` lay their table down with `migrations()`, tracked in their own ledger, `_sqlx_migrations_inbox`, as the outbox does. Each takes the clock it stamps `heard_at` with, so forgetting compares one clock with itself.

**Which listeners go through it.** *Decided 2026-11-02, the user choosing so over only the listeners whose effects are not idempotent:* every `Delivery::Kept` listener, a catalog that re-reads its aggregate as much as a command, so none has to be judged safe without one, at the cost of a row and a transaction per message heard. A sweep, which spans many transactions on purpose and is kept safe by its idempotent steps and derived continuation ids, stays a plain `messaging::Handler`, and so does a `Delivery::Fleeting` listener, which never needs one, and a listener that commits in steps on purpose, writing, committing and only then reading what a racing writer committed, so that of the two one always sees the other: in the hearing's one transaction both could read before either commits. Each of its steps is idempotent instead. A handler under `Inboxed` writes only through the transaction it is handed: one it begins itself waits, on SQLite, for the one writer, which the hearing holds. *That a Kept listener cannot be written without it, by the type system, comes once every feature's listeners are converted.*

**Conformance.** `suite.rs` holds the cases every backend passes: heard before from the second hearing on, never heard when the transaction is dropped, per listener and per message, the oldest forgotten first and no more than asked, the moment itself kept, a dropped forgetting forgetting nothing, and two handlers at once hearing a message once. `kept_tests.rs` holds `Inboxed`'s: a message handed over twice taken in once, one whose handling failed taken in anew, a lost race tried again in a fresh transaction, another refusal not tried again, and the handler named and listening as it says.
