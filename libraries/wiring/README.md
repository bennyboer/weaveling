# wiring

The seam between a feature and the application that hosts it.

A feature hands over a **`Wired`** — its routes, and the listeners it wants subscribed — and the service mounts and registers them without knowing what any of it is. What it gets back is a **`Context`**: the clock and the publisher, the two things every feature needs and none should construct.

Behind the `postgres` feature it also holds the database machinery each feature repeats: `named(feature)` for the database name, `beside(server, database)` for pointing a connection URL at a different database while keeping its query string, `ensure` to create one that is missing, `connect` for a pool, and `lay_out` to run a migrator. That is what lets a feature's own `wiring` crate expose a two-line `lay_out` instead of the composition root knowing five schemas.

Background work has no seam here yet — see [the note in the roadmap](../../ROADMAP.md#a-feature-should-be-able-to-hand-over-a-background-task).
