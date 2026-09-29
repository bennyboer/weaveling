# messaging

The messaging **seam** — the port, the envelope and an in-process dispatcher. Not a broker, and deliberately so.

A `Message` carries its own identity: a `MessageId`, the `Conversation` it belongs to, and what it was `caused_by`. Those exist from the first message rather than being added when a broker arrives, because an id minted late cannot recognise a redelivery of something sent early, and a conversation cannot be reconstructed after the fact.

A `Listener` declares what it subscribes to (`RoutingKey` and wildcard `Subscription`, `piece.#`) and how a refusal should be treated:

- **`Delivery::Kept`** — this message matters; a refusal goes to `DeadLetters` to be retried.
- **`Delivery::Fleeting`** — this message is disposable; a refusal is logged and dropped.

`InProcessDispatcher` is the only transport today: publishing awaits every interested listener in turn. A broker is an adapter for when a second deployable exists — see [why the outbox comes first](../../ARCHITECTURE.md#messaging--the-seam-now-the-transport-later).

**`InProcessDispatcher` is an exchange; `Deliveries` is the queues; `DeliveryConsumer` is the consumer.** `publish` writes one delivery row per interested listener and returns — it reports the transport and never the handling, because over a broker consumers have not run yet and may be on another machine. A `DeliveryConsumer` claims what is due and settles each delivery: handled deletes it, refused counts the attempt and pushes it out by a growing backoff, and the fifth refusal moves it to the dead letters with its root cause. So **a retry reaches only the listener that refused**, exactly as a consumer acks its own queue.

**A listener's name is its queue name**, so two listeners may not share one — `listen` asserts against it, because a duplicate would quietly eat the other's messages and that is a wiring fault worth failing at startup.

`Deliveries` has an in-memory and a PostgreSQL adapter behind one conformance suite. The state is durable on purpose: an attempt count that resets on restart is not a budget.
