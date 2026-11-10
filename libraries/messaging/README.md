# messaging

The messaging **seam** — the port, the envelope and an in-process dispatcher. Not a broker, and deliberately so.

A `Message` carries its own identity: a `MessageId`, the `Conversation` it belongs to, and what it was `caused_by`. Those exist from the first message rather than being added when a broker arrives, because an id minted late cannot recognise a redelivery of something sent early, and a conversation cannot be reconstructed after the fact.

The boards discard a deleted project's board with a `Handler`, and the dispatcher hands it what it listens to:

```rust
use std::sync::Arc;

use async_trait::async_trait;
use messaging::{Failure, Handler, Message, WithoutInbox};
use projects_contract::{DELETED, ProjectEventDTO};

#[async_trait]
impl Handler for DiscardBoardsOnProjectDeletedListener {
    const NAME: &'static str = "discard-boards-of-deleted-project";
    const LISTENS_TO: &'static [&'static str] = &[DELETED];
    const WHEN_REFUSED: &'static str = "The board of a deleted project was not cleared away.";
    const WITHOUT_INBOX: WithoutInbox = WithoutInbox::Sweep;

    async fn take_in(&self, message: &Message) -> Result<(), Failure> {
        let deleted = eventpublishing::published_in::<ProjectEventDTO>(message)?;

        Ok(self.boards.sweep(&deleted.aggregate.id).await?)
    }
}

dispatcher.listen(Arc::new(DiscardBoardsOnProjectDeletedListener::new(boards)));
```

A `Listener` declares what it subscribes to (`RoutingKey` and wildcard `Subscription`, `idea.#`) and how a refusal should be treated:

- **`Delivery::Kept`** — this message matters; a refusal goes to `DeadLetters` to be retried.
- **`Delivery::Fleeting`** — this message is disposable; a refusal is logged and dropped.

**A feature's listener without an inbox is a `Handler`.** It states its name, the routing keys it listens to and its refusal as constants, and does its work in `take_in`, which may fail with any error; the library makes a `Listener` of it and wraps a failure in a `NotHandled` naming the listener and the message. So a listener is only what it does. *It also says why it has no inbox, `WITHOUT_INBOX`, the user choosing the type system over convention 2026-11-02:* `WithoutInbox::Sweep`, a sweep spanning many transactions on purpose, its steps idempotent, delivered kept, or `WithoutInbox::Fleeting`, delivered fleeting. A kept listener that is neither is an [`inbox::KeptHandler`](../inbox/README.md), which only becomes a `Listener` through `inbox::Inboxed`, so a `Handler` that forgets its inbox does not compile. `Listener` itself stays for what a `Handler` cannot be: a fleeting broadcast, or one that wraps another, as `Inboxed` does; a hand-written `Listener` may go without an inbox as it likes, on purpose.

`InProcessDispatcher` is the only transport today: publishing awaits every interested listener in turn. A broker is an adapter for when a second deployable exists — see [why the outbox comes first](../../ARCHITECTURE.md#messaging--the-seam-now-the-transport-later).

**`InProcessDispatcher` is an exchange; `Deliveries` is the queues; `DeliveryConsumer` is the consumer.** `publish` writes one delivery row per interested listener and returns — it reports the transport and never the handling, because over a broker consumers have not run yet and may be on another machine. A `DeliveryConsumer` claims what is due and settles each delivery: handled deletes it, refused counts the attempt and pushes it out by a growing backoff, and the fifth refusal moves it to the dead letters with its root cause and the moment it was given up on. So **a retry reaches only the listener that refused**, exactly as a consumer acks its own queue. **Only the holder of a claim settles it.** *Fixed 2026-11-02, from the transaction audit:* every `claim_due` draws a `Claim`, written on each delivery it claims and carried in `Queued`; `mark_as_handled`, `mark_as_refused` and `give_up` take the `Queued` and match its claim, so a consumer whose claim ran out while it worked, the delivery claimed again by another, acks, refuses and buries nothing, and a refusal clears no claim someone else still holds. The next attempt is due counted from the refusal, not from when the batch was claimed. Leases are still measured by the app's clock, not the database's; with one clock per instance and claims of thirty seconds, a skew between instances only shortens or lengthens a lease. **A delivery waits once.** `deliveries` is unique by listener and message id, and enqueueing or retrying a dead letter whose message already waits for that listener does nothing, so a message relayed twice, published again after a failure between publishing and marking, is handled once while it waits; once handled its row is gone, and a later duplicate is the [inbox](../inbox/README.md)'s to recognise.

**A dead letter can be retried or acknowledged, and is deleted only once acknowledged and too old to try.** Retrying moves it back into the deliveries, due at once with the whole attempt budget again, and wakes the consumer. Acknowledging marks it seen and keeps it: the listener has still not handled the message, and nothing can yet rebuild what a listener never saw, so throwing it away would make the gap permanent. Both ignore an id that is already gone, because a double click is not an error. A message older than **`RETRIABLE_FOR`**, 30 days, is not to be retried, `DeadLetter::may_be_tried_again(now)` says so, and the service refuses it as `too-late-to-try-again`: features forget their tombstones after as long, so what such a message is about may be gone for good. The service serves them as `/api/service/refusals`, with the shape in `messaging-contract`, so the author can act on what was refused. **A dead letter too old to try is forgotten:** `Deliveries::forget_old_dead_letters(before, at_most)` deletes, of those whose message happened before then, the acknowledged ones whole, their alarm heard, and sets the payload of the others to `null`, keeping the letter, its listener, routing key, why and when, which is all the refusals show, until someone acknowledges it; the chore `forget-old-dead-letters` in [`chores`](../chores/README.md), which the service runs, does it every hour for those past `RETRIABLE_FOR`, so a name or address a message carried is not kept for good with it (*9d-6, 2026-11-10, as the user decided*).

**A listener says what its refusal costs the author.** `WHEN_REFUSED` has no default, so a listener cannot be written without a plain sentence — "A change to an idea did not reach the list of ideas." — and the sentence sits beside the code it describes rather than in a list somewhere that goes stale. The dispatcher answers it by listener name, and `/api/service/refusals` sends it along; a dead letter whose listener is no longer wired gets none, and the client says something general instead.

**`FakePublisher` stands in for a transport in tests.** It keeps what it is handed, and refuses everything once told to, so a test can read what was published or make a relay retry. It is plain `pub` for the reason `clock::FixedClock` is, since `cfg(test)` does not cross crate boundaries, and tests in seven crates had each written their own.

**A refusal says how a message was misshapen, never what it held.** Messages are kept and a refusal's cause is shown, and serde quotes the value it choked on, so `payload_as` and `eventpublishing::published_in` give their cause as a `Misshapen`: serde's sentence with every quoted value blanked to `"…"`, field and variant names, which serde writes in backticks, kept. Personal data may travel in a message only under a `personal` field, and only what the receiving feature needs.

**A listener's name is its queue name**, so two listeners may not share one — `listen` asserts against it, because a duplicate would quietly eat the other's messages and that is a wiring fault worth failing at startup.

**The consumer is woken, not polled.** `enqueue` notifies — a `tokio::sync::Notify` in memory and on SQLite, `pg_notify` on PostgreSQL — so a new message is taken at once and the 200ms look is a backstop for deliveries that became due again on their own. Without it the retry curve would be decorative: a delivery re-due in 200ms would still wait for the next tick.

`Deliveries` has an in-memory, a PostgreSQL and a SQLite adapter behind one conformance suite. The state is durable on purpose: an attempt count that resets on restart is not a budget. **Due deliveries come back in the order they were enqueued**: a claim returns rows in whatever order the database likes, so every adapter sorts by delivery, or a listener could be handed a detach before the attach it undoes.

**On SQLite the consumer is still woken**, unlike the outbox's relay: `enqueue` here does its own write rather than riding in someone else's transaction, so it can notify once the row is in. Giving up is two statements in one transaction — SQLite cannot modify data inside a `WITH` — and its schema is at `messaging::sqlite::migrations()`.
