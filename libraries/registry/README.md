# registry

One authoritative answer to "which id owns this key", for the cases where a projection cannot be trusted to give it.

`claim(kind, key, id)` writes the claim if the key is free and returns whoever already holds it if it is not — so the **loser of a race is told the winner's id rather than being refused**, and can go and read that one. It never changes hands.

**It exists because find-or-start was broken.** `BoardService::open` and `OutlineService::open` used to ask the catalog whether the project already had a board; the catalog is a projection fed by a message, so a second open arriving before the projection landed started a *second* board. Inline publishing hid it until [M11b](../../ROADMAP.md#milestone-11b--one-flow-in-every-mode) made both modes take the same path. A claim cannot lag the way a projection can.

**Each feature lays the table down in its own database**, so this is a shared mechanism rather than shared data — `kind` keeps a project's board and its outline from shadowing one another within one feature's schema. The migration ledger is `_sqlx_migrations_claims`, because a feature runs the event store, its catalog and this into one schema and sqlx would otherwise see one version 1 modified into another.

Two adapters, one conformance suite. The PostgreSQL one races eight concurrent claims at a single key and asserts all eight are told the same id.
