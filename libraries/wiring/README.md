# wiring

The seam between a feature and the application that hosts it.

A feature implements **`Feature`**: its name, its ports for each `Storage`, its schema, its outbox if it has one, and how to `wire` itself into a **`Wired`** — routes and the listeners it wants subscribed. **`assemble::<F>(storage, context)`** does the rest the same way for every feature — picks the ports, readies the database and lays the schema down — and returns an **`Assembled { outbox, routes, listeners }`**, so the service names a feature once and reads everything else off the list. What a feature is handed is a **`Context`**: the clock and the publisher, the two things every feature needs and none should construct. See [one value per feature](../../ARCHITECTURE.md#one-value-per-feature).

Behind the `postgres` feature it also holds the database machinery: the **`Databases`** port, which hands a feature its pool, with **`ServerDatabases`** as the production answer — `ensure` the feature's database exists, then `connect`. Underneath are `named(feature)` for the database name, `beside(server, database)` for pointing a connection URL at a different database while keeping its query string, and `lay_out` to run a migrator. **`Unprepared`** says what went wrong, including a backend this build left out.

Behind the `sqlite` feature, `wiring::sqlite` is local mode's half: one file per feature, `{feature}.sqlite`, in a data directory it creates if missing, opened with a write-ahead journal so readers never wait for the writer, foreign keys on — SQLite ignores them unless every connection asks — and a busy timeout for the moment two writers meet. Synchronous mode stays at SQLite's default, full, because what is in these files is an author's work.

Background work has no seam here yet — see [the note in the roadmap](../../ROADMAP.md#a-feature-should-be-able-to-hand-over-a-background-task).
