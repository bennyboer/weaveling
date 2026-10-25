# projects

A project is the book an author is working on — the thing everything else hangs off.

It holds almost nothing: a name, when it began, and when it was last touched. What it is *for* is identity and deletion. Every idea, board, outline and scene names a project, and deleting one has to leave nothing behind.

**`Project` is an aggregate** — `Started`, `Renamed`, `Deleted` — event-sourced like every other feature since [M11a](../../ROADMAP.md#step-1--projects-becomes-an-aggregate--done). Deletion is why: it is exactly where an author wants history, and `project.deleted` on the wire is what the cascade listens for. A deleted project is gone from the listing and refuses further changes, but its stream stays and still says what it was called and who ended it.

Unusually, the aggregate carries its own `created_at` and `updated_at`, because the project list shows an author when they last touched each one. They come from each event's `occurred_at` — never a clock the aggregate reaches for — and travel inside the snapshot body, since a snapshot is written long after the fact.

**Each sees only their own projects.** The list asks the [`Authorizer`](../../libraries/authorizing) once what the one asking may read, `Everything` in local mode or `Only` the projects they are in on a server, and the catalog answers `among` those, newest first. Who is in which project the projects do not know; the members do.

**The channel tells each their own.** `/projects/live` without a project is sifted per follower: a project's change passes if they may read it, asked of the `Authorizer` frame by frame. With `?project=` one project is followed only by whoever may read it, and is not found for anyone else. Being let into a project or out of it is the members' to tell, on [their own channel](../members).

**Crates:** `core` (aggregate, catalog port, service) · `contract` (DTOs and routing keys, shared with the client) · `adapters/store` (stored-event codec) · `adapters/catalog` (in memory, PostgreSQL and SQLite listings) · `adapters/messaging` (event publisher and catalog projector) · `adapters/rest` · `wiring` · `tests`.
