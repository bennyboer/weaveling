# wiring

The seam between a feature and the application that hosts it.

A feature implements **`Feature`**. It provides:

- its name,
- its ports for each `Storage`,
- its schema,
- its outbox, if it has one,
- how to `wire` itself into a **`Wired`**: its routes, and the listeners it wants subscribed.

**`assemble::<F>(storage, context)`** does the rest, the same way for every feature. It picks the ports, prepares the database and creates the schema. It returns an **`Assembled { outbox, routes, open_routes, topics, listeners, settling, chores }`**. So the service names a feature once, and reads everything else from that list.

A feature receives a **`Context`**. It holds the clock, the publisher and the [`Authorizer`](../authorizing). These are the things every feature needs and no feature should construct itself. The server builds the members feature first, since it has nobody to ask. Then it hands the members' `Authorizer` to every other feature. Without accounts, the `Authorizer` is `AllowingEverything`.

An event-sourced feature builds its event store and its outbox in one call: **`EventSourced::in_memory`**, `on_postgres` or `on_sqlite`. The call receives the feature's codec and its `message_for`. So on every backend, the store enqueues into the outbox next to it, and no feature can pair them differently. See [one value per feature](../../ARCHITECTURE.md#one-value-per-feature).

The ideas feature is a `Feature`, and the service assembles it like every other feature:

```rust
use std::sync::Arc;

use outbox::Outbox;
use wiring::{Context, EventSourced, Feature, Wired};

impl Feature for IdeaFeature {
    const NAME: &'static str = "ideas";

    type Ports = Ports;

    fn in_memory(context: &Context) -> Ports {
        let EventSourced { events, transactions, outbox } =
            EventSourced::in_memory(ideas_messaging::message_for, context);

        Ports { events, transactions, catalog: Arc::new(InMemoryIdeaCatalog::new()), outbox }
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

Behind the `postgres` feature, it also holds the database machinery. The **`Databases`** port hands a feature its pool. **`ServerDatabases`** is the production implementation: it makes sure (`ensure`) the feature's database exists, and then connects (`connect`). Underneath are these helpers:

- `named(feature)` returns the database name.
- `beside(server, database)` points a connection URL at a different database, and keeps its query string.
- `lay_out` runs a migrator.

**`Unprepared`** says what went wrong. That includes a backend that this build left out.

Behind the `sqlite` feature, `wiring::sqlite` is the local mode half. `Storage::Sqlite` carries a **`Files`** port. It is the twin of `Databases`, with **`DataDirectory`** as the production implementation. A feature implements `on_sqlite` and `sqlite_schema`, next to its PostgreSQL pair, and `assemble` treats the two the same way.

Underneath, there is one file per feature, `{feature}.sqlite`. It lives in a data directory, which is created if it is missing. Each file is opened with:

- a write-ahead journal, so readers never wait for the writer,
- foreign keys on, because SQLite ignores them unless every connection asks for them,
- a busy timeout, for the moment two writers meet.

Synchronous mode stays at SQLite's default, full, because these files hold an author's work.

**Background work is a [`Chore`](../chores).** A feature hands over its chores the same way it hands over its listeners, through `Wired::doing_chores`. They come out in `Assembled::chores`, and the service starts them. Work that must happen before the server stops, instead of from time to time, is `Settling`. It is handed over with `settling_before_stopping`.

**What may be followed live** is handed over as [`live`](../live) `Topic`s, through `Wired::followed_live`. The server serves all of them on its one socket, `/api/live`.

**Routes for anyone**, signed in or not, are handed over separately, through `Wired::serving_anyone`. The server serves them next to its own open routes, outside the sign-in guard that protects every other route. So far there is only one such route: the link in an invitation to decline it.
