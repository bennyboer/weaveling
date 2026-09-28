# messaging

The messaging **seam** — the port, the envelope and an in-process dispatcher. Not a broker, and deliberately so.

A `Message` carries its own identity: a `MessageId`, the `Conversation` it belongs to, and what it was `caused_by`. Those exist from the first message rather than being added when a broker arrives, because an id minted late cannot recognise a redelivery of something sent early, and a conversation cannot be reconstructed after the fact.

A `Listener` declares what it subscribes to (`RoutingKey` and wildcard `Subscription`, `piece.#`) and how a refusal should be treated:

- **`Delivery::Kept`** — this message matters; a refusal goes to `DeadLetters` to be retried.
- **`Delivery::Fleeting`** — this message is disposable; a refusal is logged and dropped.

`InProcessDispatcher` is the only transport today: publishing awaits every interested listener in turn. A broker is an adapter for when a second deployable exists — see [why the outbox comes first](../../ARCHITECTURE.md#messaging--the-seam-now-the-transport-later).

**Known gap:** `publish` reports success whatever the listeners do, so a `Kept` refusal is logged rather than retried, even behind an outbox. [M11b](../../ROADMAP.md#milestone-11b--one-flow-in-every-mode) closes it.
