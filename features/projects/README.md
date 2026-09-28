# projects

A project is the book an author is working on — the thing everything else hangs off.

It holds almost nothing: a name, when it began, and when it was last touched. What it is *for* is identity and deletion. Every piece, board, outline and passage names a project, and deleting one has to leave nothing behind.

**`Project` is an aggregate** — `Started`, `Renamed`, `Deleted` — event-sourced like every other feature since [M11a](../../ROADMAP.md#step-1--projects-becomes-an-aggregate--done). Deletion is why: it is exactly where an author wants history, and `project.deleted` on the wire is what the cascade listens for. A deleted project is gone from the listing and refuses further changes, but its stream stays and still says what it was called and who ended it.

Unusually, the aggregate carries its own `created_at` and `updated_at`, because the project list shows an author when they last touched each one. They come from each event's `occurred_at` — never a clock the aggregate reaches for — and travel inside the snapshot body, since a snapshot is written long after the fact.

**Crates:** `core` (aggregate, catalog port, service) · `contract` (DTOs and routing keys, shared with the client) · `adapters/store` (stored-event codec) · `adapters/catalog` (in-memory and PostgreSQL listings) · `adapters/messaging` (event publisher and catalog projector) · `adapters/rest` · `wiring` · `tests`.
