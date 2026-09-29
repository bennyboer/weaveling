# messaging

The messaging **seam** — the port, the envelope and an in-process dispatcher. Not a broker, and deliberately so.

A `Message` carries its own identity: a `MessageId`, the `Conversation` it belongs to, and what it was `caused_by`. Those exist from the first message rather than being added when a broker arrives, because an id minted late cannot recognise a redelivery of something sent early, and a conversation cannot be reconstructed after the fact.

A `Listener` declares what it subscribes to (`RoutingKey` and wildcard `Subscription`, `piece.#`) and how a refusal should be treated:

- **`Delivery::Kept`** — this message matters; a refusal goes to `DeadLetters` to be retried.
- **`Delivery::Fleeting`** — this message is disposable; a refusal is logged and dropped.

`InProcessDispatcher` is the only transport today: publishing awaits every interested listener in turn. A broker is an adapter for when a second deployable exists — see [why the outbox comes first](../../ARCHITECTURE.md#messaging--the-seam-now-the-transport-later).

**`publish` reports the transport, never the handling.** It says whether the message was handed over — over a broker, whether the exchange took it. It cannot say more: consumers have not run yet and may be on another machine. A `Kept` refusal therefore reaches `DeadLetters` and nothing retries it, which is the gap [M11b step 2](../../ROADMAP.md#milestone-11b--one-flow-in-every-mode) has to close **on the consuming side**, where a broker closes it — a queue per listener, a bounded number of redeliveries, then a dead-letter queue.
