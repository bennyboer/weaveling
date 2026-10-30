# wiring

The seam between a feature and the application that hosts it.

A feature implements **`Feature`**: its name, its ports for each `Storage`, its schema, its outbox if it has one, and how to `wire` itself into a **`Wired`** — routes and the listeners it wants subscribed. **`assemble::<F>(storage, context)`** does the rest the same way for every feature — picks the ports, readies the database and lays the schema down — and returns an **`Assembled { outbox, routes, open_routes, topics, listeners, settling, chores }`**, so the service names a feature once and reads everything else off the list. What a feature is handed is a **`Context`**: the clock, the publisher and the [`Authorizer`](../authorizing), the things every feature needs and none should construct. The server builds the members first, with no one to ask, and hands their `Authorizer` to every other feature; without accounts it is `AllowingEverything`. An event-sourced feature builds its event store and its outbox in one call, **`EventSourced::in_memory`**, `on_postgres` or `on_sqlite`, given its codec and its `message_for`, so the store enqueues into the outbox beside it on every backend and no feature can pair them differently. See [one value per feature](../../ARCHITECTURE.md#one-value-per-feature).

The ideas are a `Feature`, and the service assembles them like every other:

```rust
use std::sync::Arc;

use outbox::Outbox;
use wiring::{Context, EventSourced, Feature, Wired};

impl Feature for IdeaFeature {
    const NAME: &'static str = "ideas";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced { events, outbox } =
            EventSourced::in_memory(ideas_messaging::message_for, context);

        Ports { events, catalog: Arc::new(InMemoryIdeaCatalog::new()), outbox }
    }

    fn outbox(ports: &Ports) -> Option<Arc<dyn Outbox>> {
        Some(ports.outbox.clone())
    }

    fn wire(ports: &Ports, context: &Context) -> Wired {
        Wired::serving(ideas_rest::router(core(ports, context)))
    }
}

let ideas = wiring::assemble::<IdeaFeature>(&storage, &context).await?;
```

Behind the `postgres` feature it also holds the database machinery: the **`Databases`** port, which hands a feature its pool, with **`ServerDatabases`** as the production answer — `ensure` the feature's database exists, then `connect`. Underneath are `named(feature)` for the database name, `beside(server, database)` for pointing a connection URL at a different database while keeping its query string, and `lay_out` to run a migrator. **`Unprepared`** says what went wrong, including a backend this build left out.

Behind the `sqlite` feature, `wiring::sqlite` is local mode's half. `Storage::Sqlite` carries a **`Files`** port, the twin of `Databases`, with **`DataDirectory`** as the production answer; a feature implements `on_sqlite` and `sqlite_schema` beside its PostgreSQL pair, and `assemble` treats the two alike. Underneath it is one file per feature, `{feature}.sqlite`, in a data directory it creates if missing, opened with a write-ahead journal so readers never wait for the writer, foreign keys on — SQLite ignores them unless every connection asks — and a busy timeout for the moment two writers meet. Synchronous mode stays at SQLite's default, full, because what is in these files is an author's work.

**Background work is a [`Chore`](../chores)**: a feature hands its chores over as it does its listeners, `Wired::doing_chores`, and they come out in `Assembled::chores` for the service to start. Work that must happen before the server stops, rather than now and then, is `Settling`, handed over by `settling_before_stopping`. **What may be followed live** is handed over as [`live`](../live) `Topic`s, `Wired::followed_live`, and the server serves them all on its one socket, `/api/live`. **Routes for anyone**, signed in or not, are handed over apart, `Wired::serving_anyone`, and the server serves them beside its own open routes, outside the sign-in guard that keeps every other route; so far only the invitations' link to turn one down.
