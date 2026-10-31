# inbox

What a listener has heard already, so a message handed over twice is taken in once. The dual of [the outbox](../outbox/README.md).

A message can arrive at a listener more than once: the relay may publish it again after a crash between publishing and marking it published, and a delivery is tried again after a handler failed. A listener whose effects must not happen twice asks its feature's **`Inbox`** first, inside the same [`Transaction`](../transacting/README.md) as those effects:

```rust
let mut transaction = self.transactions.begin().await?;
if !self.inbox.first_time(&mut transaction, &self.named(), message.id).await? {
    return Ok(());
}
self.notifications.note(Some(&mut transaction), account, about).await?;
transaction.finish().await?;
```

**`first_time(transaction, listener, message)`** records `(listener, message id)` and answers whether this listener had heard this message before. The record commits with the handler's effects or rolls back with them, so a handler that failed hears its message again when it is redelivered, and one that succeeded never does. It is per listener: two listeners hear one message each for themselves. Two handlers hearing one message at once hear it once; the second waits for the first's transaction and then finds it heard.

**Kept for a while, then forgotten.** A message is not handed over again once it is older than `messaging::RETRIABLE_FOR`, so what was heard need not be kept longer either. **`forget_heard_before(transaction, before, at_most)`** forgets the oldest first, no more than asked, for a chore to run, as tombstones are forgotten.

**Where it lives.** In the feature's own database, beside its other ports, since it is only worth something inside the handler's transaction; the deliveries, in the `messaging` database, cannot commit with it. **`InMemoryInbox`** undoes what it recorded when the transaction is dropped, **`PostgresInbox`** behind the `postgres` feature and **`SqliteInbox`** behind `sqlite` lay their table down with `migrations()`, tracked in their own ledger, `_sqlx_migrations_inbox`, as the outbox does. Each takes the clock it stamps `heard_at` with, so forgetting compares one clock with itself.

**Which listeners need it.** Only those whose effects are not idempotent on their own. A catalog that re-reads the aggregate, or a command the aggregate refuses or ignores the second time, takes a redelivery in its stride without one. A `Delivery::Fleeting` listener never needs one.

**Conformance.** `suite.rs` holds the cases every backend passes: heard the first time only, never heard when the transaction is dropped, per listener and per message, the oldest forgotten first and no more than asked, the moment itself kept, a dropped forgetting forgetting nothing, and two handlers at once hearing a message once.
