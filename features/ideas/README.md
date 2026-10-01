# ideas

An idea is one unit of an author's thinking — captured first, arranged later.

This is where the model lives: **ideas are a pool, and every view is an arrangement of them**. An idea knows its project and its title, and nothing else. It does **not** know where it sits on a board or where it hangs in an outline — position belongs to the view, never to the idea. See [Ideas and views](../../ARCHITECTURE.md#ideas-and-views--the-non-linear-model).

**`Idea` is an aggregate** — `Captured`, `Retitled`, `Discarded` — and it was the first one, where event sourcing debuted. An idea may be captured untitled, because an author mid-thought should not be stopped to name things.

Discarding is the interesting edge: it leaves the idea refusing everything and drops it from the listing, and `idea.discarded` is what the board listens for so it can unpin it. That stays housekeeping rather than correctness — a client must keep tolerating a dangling reference, because any transport leaves a window.

**Crates:** `core` (aggregate, catalog port, service) · `contract` · `adapters/store` (stored-event codec) · `adapters/catalog` (listings per project) · `adapters/messaging` (event publisher and catalog projector) · `adapters/rest` · `wiring` · `tests`.
