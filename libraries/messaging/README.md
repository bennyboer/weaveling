# messaging

The messaging **seam** — the port, the envelope and an in-process dispatcher. Not a broker, and deliberately so.

A `Message` carries its own identity: a `MessageId`, the `Conversation` it belongs to, and what it was `caused_by`. Those exist from the first message rather than being added when a broker arrives, because an id minted late cannot recognise a redelivery of something sent early, and a conversation cannot be reconstructed after the fact.

A `Listener` declares what it subscribes to (`RoutingKey` and wildcard `Subscription`, `idea.#`) and how a refusal should be treated:

- **`Delivery::Kept`** — this message matters; a refusal goes to `DeadLetters` to be retried.
- **`Delivery::Fleeting`** — this message is disposable; a refusal is logged and dropped.

`InProcessDispatcher` is the only transport today: publishing awaits every interested listener in turn. A broker is an adapter for when a second deployable exists — see [why the outbox comes first](../../ARCHITECTURE.md#messaging--the-seam-now-the-transport-later).

**`InProcessDispatcher` is an exchange; `Deliveries` is the queues; `DeliveryConsumer` is the consumer.** `publish` writes one delivery row per interested listener and returns — it reports the transport and never the handling, because over a broker consumers have not run yet and may be on another machine. A `DeliveryConsumer` claims what is due and settles each delivery: handled deletes it, refused counts the attempt and pushes it out by a growing backoff, and the fifth refusal moves it to the dead letters with its root cause and the moment it was given up on. So **a retry reaches only the listener that refused**, exactly as a consumer acks its own queue.

**A dead letter can be retried or acknowledged, never deleted.** Retrying moves it back into the deliveries, due at once with the whole attempt budget again, and wakes the consumer. Acknowledging marks it seen and keeps it: the listener has still not handled the message, and nothing can yet rebuild what a listener never saw, so throwing it away would make the gap permanent. Both ignore an id that is already gone, because a double click is not an error. The service serves them as `/api/service/refusals`, with the shape in `messaging-contract`, so the author can act on what was refused.

**A listener's name is its queue name**, so two listeners may not share one — `listen` asserts against it, because a duplicate would quietly eat the other's messages and that is a wiring fault worth failing at startup.

**The consumer is woken, not polled.** `enqueue` notifies — a `tokio::sync::Notify` in memory and on SQLite, `pg_notify` on PostgreSQL — so a new message is taken at once and the 200ms look is a backstop for deliveries that became due again on their own. Without it the retry curve would be decorative: a delivery re-due in 200ms would still wait for the next tick.

`Deliveries` has an in-memory, a PostgreSQL and a SQLite adapter behind one conformance suite. The state is durable on purpose: an attempt count that resets on restart is not a budget. **Due deliveries come back in the order they were enqueued**: a claim returns rows in whatever order the database likes, so every adapter sorts by delivery, or a listener could be handed a detach before the attach it undoes.

**On SQLite the consumer is still woken**, unlike the outbox's relay: `enqueue` here does its own write rather than riding in someone else's transaction, so it can notify once the row is in. Giving up is two statements in one transaction — SQLite cannot modify data inside a `WITH` — and its schema is at `messaging::sqlite::migrations()`.
