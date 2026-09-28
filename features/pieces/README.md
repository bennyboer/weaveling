# pieces

A piece is one unit of an author's thinking — captured first, arranged later.

This is where the model lives: **pieces are a pool, and every view is an arrangement of them**. A piece knows its project, its title, and the passage holding its prose. It does **not** know where it sits on a board or where it hangs in an outline — position belongs to the view, never to the piece. See [Pieces and views](../../ARCHITECTURE.md#pieces-and-views--the-non-linear-model).

**`Piece` is an aggregate** — `Captured`, `Retitled`, `PassageAttached`, `Discarded` — and it was the first one, where event sourcing debuted. A piece may be captured untitled, because an author mid-thought should not be stopped to name things.

Discarding is the interesting edge: it leaves the piece refusing everything and drops it from the listing, and `piece.discarded` is what the board listens for so it can unpin it. That stays housekeeping rather than correctness — a client must keep tolerating a dangling reference, because any transport leaves a window.

**Crates:** `core` (aggregate, catalog port, service) · `contract` · `adapters/store` (stored-event codec) · `adapters/catalog` (listings per project) · `adapters/messaging` (event publisher and catalog projector) · `adapters/rest` · `wiring` · `tests`.
