# messaging

The messaging **seam**: the port, the envelope and an in-process dispatcher. It is not a broker, and that is deliberate.

A `Message` carries its own identity: a `MessageId`, the `Conversation` it belongs to, and what it was `caused_by`. These exist from the first message on. They are not added later, when a broker arrives. The reasons: an id created late cannot recognise a redelivery of something sent early. And a conversation cannot be reconstructed after the fact.

The boards feature discards a deleted project's board with a `Handler`. The dispatcher passes it the messages it listens to:

```rust
use std::sync::Arc;

use async_trait::async_trait;
use messaging::{Failure, Handler, Message, WithoutInbox};
use projects_contract::{DELETED, ProjectEventDTO};

#[async_trait]
impl Handler for DiscardBoardsOnProjectDeletedListener {
    const NAME: &'static str = "discard-boards-of-deleted-project";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str = "The board of a deleted project was not removed.";
    const WITHOUT_INBOX: WithoutInbox = WithoutInbox::Sweep;

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let deleted = eventpublishing::published_in::<ProjectEventDTO>(message)?;

        Ok(self.boards.sweep(&deleted.aggregate.id).await?)
    }
}

dispatcher.listen(Arc::new(DiscardBoardsOnProjectDeletedListener::new(boards)));
```

A `Listener` declares what it subscribes to (a `RoutingKey`, or a wildcard `Subscription` such as `idea.#`). It also declares how a refusal is treated:

- **`Delivery::Kept`**: this message matters. A refusal goes to `DeadLetters`, to be retried.
- **`Delivery::Fleeting`**: this message can be thrown away. A refusal is logged and dropped.

**A feature's listener without an inbox is a `Handler`.** It declares its name, the routing keys it listens to, and its refusal text as constants. It does its work in `take_in`, which may fail with any error. The library turns it into a `Listener`. It wraps a failure in a `NotHandled` that names the listener and the message. So a listener only contains what it does.

*A `Handler` also declares why it has no inbox, in `WITHOUT_INBOX`. Decided on 2026-11-02: the user chose the type system over a convention.* There are two options:

- `WithoutInbox::Sweep`: a sweep that spans many transactions on purpose. Its steps are idempotent. It is delivered as kept.
- `WithoutInbox::Fleeting`: it is delivered as fleeting.

A kept listener that is neither is an [`inbox::KeptHandler`](../inbox/README.md). A `KeptHandler` only becomes a `Listener` through `inbox::Inboxed`. So a `Handler` that forgets its inbox does not compile. `Listener` itself remains for things a `Handler` cannot be: a fleeting broadcast, or a listener that wraps another one, as `Inboxed` does. A hand-written `Listener` may go without an inbox if it wants to, on purpose.

`InProcessDispatcher` is the only transport today. Publishing awaits every interested listener in turn. A broker will be an adapter, added when a second deployable exists. See [why the outbox comes first](../../ARCHITECTURE.md#messaging--the-seam-now-the-transport-later).

**`InProcessDispatcher` is the exchange. `Deliveries` is the queues. `DeliveryConsumer` is the consumer.** `publish` writes one delivery row per interested listener and then returns. It reports on the transport, never on the handling. The reason is that with a broker, the consumers have not run yet at that point, and they may be on another machine.

A `DeliveryConsumer` claims what is due and settles each delivery:

- If the delivery was handled, it is deleted.
- If it was refused, the attempt is counted and the delivery is pushed back by a growing backoff.
- On the fifth refusal, it moves to the dead letters, with its root cause and the moment it was given up.

So **a retry reaches only the listener that refused**, just as a consumer acknowledges its own queue.

**Only the holder of a claim settles it.** *Fixed on 2026-11-02, from the transaction audit.* Every `claim_due` creates a `Claim`. The claim is written on each delivery it claims, and it is carried in `Queued`. `mark_as_handled`, `mark_as_refused` and `give_up` take the `Queued` and must match its claim. A consumer's claim may run out while it works, and another consumer may claim the delivery again. Then the first consumer can no longer acknowledge, refuse or bury anything. And a refusal does not clear a claim that someone else still holds. The next attempt is due counting from the refusal, not from when the batch was claimed.

Leases are still measured by the app's clock, not the database's. Each instance has one clock, and claims last thirty seconds. So a clock skew between instances only makes a lease shorter or longer.

**A delivery waits only once.** `deliveries` is unique per listener and message id. Enqueueing a message that is already waiting for that listener does nothing. The same is true for retrying a dead letter whose message is already waiting. So a message that was relayed twice is handled once while it waits. That happens when it was published again after a failure between publishing and marking it as published. Once the message is handled, its row is gone. A later duplicate must then be recognised by the [inbox](../inbox/README.md).

**A dead letter can be retried or acknowledged. It is deleted only when it is acknowledged and too old to retry.**

- Retrying moves it back into the deliveries. It is due at once, with the full attempt budget again, and the consumer is woken.
- Acknowledging marks it as seen and keeps it. The listener has still not handled the message. And nothing can yet rebuild what a listener never saw. So throwing it away would make the gap permanent.

Both ignore an id that is already gone, because a double click is not an error.

A message older than **`RETRIABLE_FOR`** (30 days) must not be retried. `DeadLetter::may_be_tried_again(now)` reports this, and the service refuses it as `too-late-to-try-again`. The reason: features delete their tombstones after the same time. So the thing such a message is about may be gone for good.

The service serves the dead letters at `/api/service/refusals`, with the shape from `messaging-contract`. So the author can act on what was refused.

**A dead letter that is too old to retry is deleted.** `Deliveries::forget_old_dead_letters(before, at_most)` handles the dead letters whose message happened before `before`:

- The acknowledged ones are deleted completely, because someone has seen their alarm.
- For the others, the payload is set to `null`. The letter itself is kept, with its listener, routing key, reason and time. That is all the refusals list shows. It stays until someone acknowledges it.

The chore `forget-old-dead-letters` in [`chores`](../chores/README.md) does this every hour, for dead letters older than `RETRIABLE_FOR`. The service runs this chore. So a name or address that a message carried is not kept forever with it. (*9d-6, decided by the user on 2026-11-10.*)

**A listener states what its refusal costs the author.** `WHEN_REFUSED` has no default. So a listener cannot be written without a plain sentence, such as "A change to an idea did not reach the list of ideas." The sentence sits next to the code it describes, not in a separate list that goes stale. The dispatcher returns it by listener name, and `/api/service/refusals` sends it along. A dead letter whose listener is no longer wired gets no sentence, and the client shows a general text instead.

**`FakePublisher` replaces a transport in tests.** It keeps what it receives. Once told to, it refuses everything. So a test can read what was published, or make a relay retry. It is plain `pub` for the same reason as `clock::FixedClock`: `cfg(test)` does not cross crate boundaries, and tests in seven crates had each written their own.

**A refusal says how a message was malformed, never what it contained.** Messages are kept, and a refusal's cause is shown. But serde quotes the value it failed on. So `payload_as` and `eventpublishing::published_in` return their cause as a `Misshapen`. That is serde's sentence, with every quoted value replaced by `"…"`. Field and variant names, which serde writes in backticks, are kept. Personal data may travel in a message only under a `personal` field, and only what the receiving feature needs.

**A listener's name is its queue name.** So two listeners may not share a name. `listen` asserts this. A duplicate would silently take the other listener's messages. That is a wiring fault, and it is worth failing at startup for.

**The consumer is woken, not polled.** `enqueue` sends a notification: a `tokio::sync::Notify` in memory and on SQLite, and `pg_notify` on PostgreSQL. So a new message is taken at once. The check every 200ms is only a backstop, for deliveries that became due again on their own. Without the notification, the retry curve would be useless: a delivery due again in 200ms would still wait for the next tick.

`Deliveries` has an in-memory, a PostgreSQL and a SQLite adapter, all held to one conformance suite. The state is durable on purpose: an attempt count that resets on restart is not a budget.

**Due deliveries come back in the order they were enqueued.** A claim returns rows in whatever order the database chooses. So every adapter sorts by delivery. Otherwise a listener could receive a detach before the attach it undoes.

**On SQLite the consumer is still woken**, unlike the outbox's relay. The reason: `enqueue` here does its own write, instead of riding in someone else's transaction. So it can send the notification once the row is stored. Giving up is two statements in one transaction, because SQLite cannot modify data inside a `WITH`. The schema is at `messaging::sqlite::migrations()`.
