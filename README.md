# Weaveling

> Bring us your tiny, fragile story ideas, and we will help you weave them into a full epic.

## What is Weaveling?

Weaveling is a tool for writing books — historical fiction, science-fiction, and pure fantasy alike or whatever you want!

Writers often look at their messy first drafts and think, *"This isn't a masterpiece yet, it's just a little mess."* The name **Weaveling** answers that thought directly: bring your smallest, most fragile threads of an idea, and Weaveling helps you structure them into a full, woven epic.

## Why "Weaveling"?

Stories are woven. They start as loose, tangled threads — a character, a scene, a "what if" — and become a tapestry only once those threads are structured, connected, and given shape.

The `-ling` says something too: a weaveling is small, young, and alive. It's the story in its infancy. Weaveling meets the idea where it is — tiny and unfinished — and grows with it.

## The Vision

Weaveling is built by a software developer who loves typography and art, for the author they want to become. It is opinionated about craft: the writing experience, the way text looks on the page, and the structure beneath a story all matter.

The promise to the writer:

- **No blank-page paralysis.** A first draft is *supposed* to be a little mess. Weaveling embraces that.
- **Structure without straitjackets.** Help turning tangled threads into a coherent narrative, without forcing a rigid template.
- **Typography and craft as first-class citizens.** How the words look and feel is part of the writing, not an afterthought.
- **Just-in-time detail.** Like iterative software development: you never have to specify the whole book up front. Drill deeper only where and when the idea is ready for it.

## The Workspace

Weaveling holds **multiple projects** in one place. A project has a name (a working title or the final book name) and is the container for everything below.

A project is deliberately flexible about scope: it can be a single book, or a whole multi-book series. The top of a project's outline might be one book, or a series whose children are the individual books — that choice is left to the author.

## The Core Idea: Warp & Weft

Underneath every view is a single shared model — one fabric, seen from different angles.

- The raw material comes in two kinds. An **idea** is a loose thought — a title, nothing more, and as disposable as it sounds. A **scene** is the prose itself: a stretch of the book's text, with a title if the author wants one. Neither knows where it belongs.
- The **outline** is the *warp*: the vertical threads, the spine of the book. It arranges scenes into sections, and notes ideas beside them.
- **Time**, **codex entities**, and **threads** are the *weft*: they run crossways through the scenes.
- A scene is where warp meets weft — where a character appears, in a section of the book, at a moment in time.

The views are not separate tools stapled together; they are lenses onto the same weave. This is why they stay linked: navigating from one to another is just turning the fabric to catch the light differently.

**No view owns a scene or an idea.** The same scene can sit in a section of the outline, at a moment on the timeline, and inside two subplots. It can also draw on ideas that were born on the board. None of these places is *the* answer to where the scene is. This is also why a scene or an idea can exist without ever being placed anywhere: unwoven thread is still thread.

## The Views

### Board view

Where ideas land before they are anything. It is an infinite surface, closer to a wall of index cards than to a document. The author throws an idea onto it, and may give it a title or not.

Nothing on the board has a place in the book yet. Ideas sit wherever they were dropped, in whatever clusters make sense that day, and spatial memory is allowed to do real work — *the abandoned ideas live bottom-left*. Several authors can brainstorm on one board at once and watch each other move things around.

This is the answer to blank-page paralysis. Capturing an idea demands no decision about where it belongs, so there is no structure to fight before writing a single sentence.

### Outline view

The book as a hierarchy — the shape a reader would recognise as a table of contents. The author nests sections as far as the budding book allows, places scenes in them, and notes ideas from the board beside the sections they belong to. Detail is added *just in time*, not up front: beginning, main part, ending is a perfectly good outline for a long while.

The finest grain is **the scene**: sections give the structure, scenes inside them carry the words.

**This is the privileged view.** A book is ultimately linear, so reading — and export — is an in-order walk of the outline, stitching every scene's text together. A scene that is *not* in the outline is simply not in the book yet, which is a feature rather than a gap: the board can hold twenty ideas that may never make it, and none of them has to be deleted to stay out of the way.

**Prose lives in scenes, never in sections.** A section is structure only; its words are the scenes in it. So reshaping the book moves scenes around rather than cutting text apart.

### Writing view

Where the actual prose is written. Deliberately **distraction-free**: just the text, in a large, comfortable font, so the author can concentrate on the words — the book's scenes one after another, under their section names, as one flow to read and write in.

It is linked to every other view. A scene can be opened from the outline, from an idea that draws it in, or from the timeline. Opening it lands right at that scene in the flow. Codex entity names (see below) are **highlighted** inline, so their sheet is one glance away while writing dialogue.

### Codex

Every book has a cast — and a world. The **Codex** is the compendium of everything in the story: **characters** first and foremost, but also **locations, factions, magic/tech systems, artifacts**, and anything else worth tracking. They share one shape: fields, images, milestones, relations, free text, and back-links to the scenes they appear in.

The Codex is linked to the other views:

- Each entity keeps an **alias list** (names, titles, nicknames), used to detect mentions in the prose.
- In the **writing view**, entity names are **highlighted**, so their entry is one glance away.
- A scene shows which entities appear in it — **detected from the text, or attached manually** by the author.
- From an entity's entry, the author can see every scene it appears in, and jump straight there.

### Timeline

Especially for historical novels, the fictional story is woven around events that really happened in time. The author can start *in the timeline*, create scenes from there (they join the pool like any other scene, and reach the outline only when the author is ready), and arrange them in correct chronological relation.

Time in Weaveling is an **abstract order**, not a calendar:

- Scenes carry **before / after** relations to one another — like a linked list, but scenes may also sit **in parallel** (happening at the same time).
- Scenes can be tagged into **time buckets** of any precision — "the year 1737", "April 24th 1737", or fuzzy ones like "early spring". Buckets give a place to attach time information without forcing exact dates, so the same model serves real history and invented calendars alike.

Because narrative order and chronological order differ (flashbacks, non-linear storytelling), the timeline is a genuine second ordering over the same scenes — not just a re-sort of the outline.

### Threads

A **thread** is a colored strand followed through the manuscript — a subplot, a motif, a mystery and its clues. Overlaid on the outline, a thread shows which scenes carry it, and where it goes quiet for too long. This is the most literally *Weaveling* view: it lets the author see and tend the individual strands running through the weave.

### Research / Sources

A place to collect references, quotes, images, and links — especially for historical work — and pin them to the scenes and ideas they inform. Distinct from the Codex: the Codex is the author's invented world; Research is the real-world material the story draws from.

## Export

Typesetting the finished weave is an **export function**, not a view. The in-order walk of the outline is rendered into a polished artifact — **PDF, EPUB**, or other formats — the moment the mess becomes a book. This is where the typography obsession pays off.

## How It's Built

Weaveling is a browser-based client–server app. The backend is a Rust modular monolith. Structure is stored with event sourcing, and prose with CRDTs. That way, editing is real-time collaborative *and* works offline. The data is stored in PostgreSQL, behind ports that do not know that PostgreSQL is there. See [ARCHITECTURE.md](./ARCHITECTURE.md) for the full picture, [ROADMAP.md](./ROADMAP.md) for what's actually being built right now, and [CONVENTIONS.md](./CONVENTIONS.md) for how it is written.

## Getting Started

### Prerequisites

- **Rust**, via [rustup](https://rustup.rs). The toolchain, the `rustfmt`/`clippy` components and the `wasm32-unknown-unknown` target are all declared in `rust-toolchain.toml`. They are installed automatically on the first build, so there is nothing to add by hand.
- **[cargo-leptos](https://github.com/leptos-rs/cargo-leptos)**, which builds the web client and the server that serves it. Cargo cannot declare tool dependencies, so this one must be installed by hand:

  ```bash
  cargo install --locked cargo-leptos
  ```

- **Node**, only for the client's end-to-end tests. It is not needed to build or run anything:

  ```bash
  cd clients/web && npm install && npx playwright install chromium
  ```

- **Docker**, only for the tests that run against a real PostgreSQL. `cargo nextest run` without those tests needs nothing:

  ```bash
  docker compose up -d
  ```

  On the first start, it creates one database per feature. It reads the list of features from `features/`, so the list is not repeated. PostgreSQL runs its init scripts only on an empty data directory. So a feature that is added later does not get a database this way. Until the API takes over creating the databases at startup, run `docker compose down -v` and then start it up again.

### Running it for development

Development runs as one process. The API serves the client next to `/api`. [cargo-leptos](https://github.com/leptos-rs/cargo-leptos) builds both and watches them for changes. Install it once with `cargo install --locked cargo-leptos`. It needs `node` on the path, for the editor's JavaScript. If you installed a `wasm-bindgen` of your own, it must be the version that `Cargo.lock` names.

```bash
cargo dev
```

`cargo dev` is an alias for `cargo leptos watch --split`, defined in `.cargo/config.toml`. Then **open http://localhost:3000**. Always pass `--split` when you call cargo-leptos yourself. The views are loaded only when they are opened (`clients/web/src/app/lazy.rs`). A build without `--split` shows a blank page, because its code asks for a module that only the split build makes. While it runs:

- a change to the client rebuilds the client and reloads the page,
- a change to the styles swaps them in without a reload,
- a change to the server rebuilds and restarts the server, and then reloads the page.

Arguments for the server go after `--`, as in `cargo dev -- --config my.toml`.

`TraceLayer` logs at DEBUG level, but `tracing_subscriber` defaults to INFO. So you get no request log unless you ask for one:

```bash
RUST_LOG=info,tower_http=debug cargo dev
```

The API alone, without the client, still runs as `cargo run -p weaveling-service-api`.

### Running a release build

Build the client and the server next to it. The client is split by view and sized for the web:

```bash
cargo build-release
```

`cargo build-release` is an alias for `cargo leptos build --release --split`, defined in `.cargo/config.toml`. It writes the client to `target/site`: `page.html`, the styles, fonts and icons, and `pkg/` with the WebAssembly chunks. It writes the server to `target/release/weaveling-service-api` (`.exe` on Windows). The two belong together: copy both to wherever the instance runs. Then start the server and name the client's folder:

```bash
WEAVELING_CLIENT_DIR=target/site ./target/release/weaveling-service-api --config weaveling.toml
```

Instead of the variable, you can set `client_dir = "target/site"` under `[server]` in the settings file. Without either, the server answers only on `/api` and serves no client.

The server listens on `127.0.0.1:3000`, unless it is configured otherwise (see below). Behind a proxy, set `public_url` to the address that people open. Otherwise no passkey can be added. The files are served uncompressed, so let the proxy in front compress them, with gzip or brotli. The first download is about 1.9 MB uncompressed and 0.5 MB compressed.

**Settings are read once, at startup.** They come from these sources, and a later source wins over an earlier one:

1. the built-in defaults,
2. a `weaveling.toml` next to the server, or the file that `--config <path>` or `WEAVELING_CONFIG` names,
3. the environment variables below.

[`weaveling.example.toml`](./weaveling.example.toml) lists every key, each with the variable that has the same meaning. If the server cannot read a value, it stops. It then says which key in which file, or which variable, holds that value. The API listens on `127.0.0.1:3000`, unless `WEAVELING_LISTEN` (`server.listen` in the file) names another address and port. Use `0.0.0.0:3000` to make it reachable from other machines.

**For running it by hand**, run `node scripts/dev-settings.mjs`. It writes two files:

- a `weaveling.toml` of your own, with `public_url = "http://localhost:3000"`,
- a `weaveling.dev.key`, which that `weaveling.toml` names. The key seals the secrets of two-step sign-in.

Because of that `public_url`, open the client at localhost, not at 127.0.0.1, or passkeys will not work. Git ignores both files. The script never overwrites either of them, because a new key locks out every second factor that was set up with the old key. The browser tests start the API with `--config clients/web/e2e/weaveling.toml` instead. That file has a key only for tests, so your own file changes nothing in the tests.

**Where the work is stored is chosen at startup.** One binary supports every option:

- **Nothing set** — in memory, so restarting the API empties it.
- **`WEAVELING_DATA=<directory>`** — local mode: one SQLite file per feature, in that directory. The directory is created if it is missing. The work survives a restart, and it is a set of files that you own.
- **`WEAVELING_DATABASE_URL=<server>`** — PostgreSQL.

Setting both is refused. The server does not guess which one you meant. Both names have the `WEAVELING_` prefix on purpose: a `DATABASE_URL` exported for some other project must never decide where an author's work goes. Only the test harness reads the plain `DATABASE_URL`. The harness is developer tooling, so it follows the common convention.

```bash
WEAVELING_DATA=~/weaveling cargo run -p weaveling-service-api

docker compose up -d
WEAVELING_DATABASE_URL=postgres://weaveling:weaveling@127.0.0.1:5432/weaveling cargo run -p weaveling-service-api
```

In both cases, the API creates every schema at startup. It also runs one outbox relay for each feature that publishes events. On PostgreSQL, each relay is woken by a notification, and it also polls every five seconds as a backstop. SQLite has no notifications, so in local mode the relays poll every 100ms instead. `ctrl-c` stops the relays before the API exits.

**`WEAVELING_FLAKY=<share>`** makes every listener refuse a share of the messages it receives. The share is a number from `0` to `1`. This lets you try the alarm and its dialog by hand. A message is given up only after five refusals in a row. With the backoff, that takes about half a minute. So at `0.5`, only one in 32 deliveries ends as a refusal. At `0.8`, about a third of them do. This setting is for trying things out, never for an author. The log warns at startup when it is set.

### Testing the client

The client is tested end to end in a real browser. The reason: the two client bugs that actually shipped were about browser behavior, not about logic. See [Client conventions](./ARCHITECTURE.md#client-conventions).

```bash
cd clients/web
npm test
```

Playwright starts its own test server with `cargo leptos serve --split`. That server runs at :3100 and serves from `target/site-tests`. So a `cargo dev` of yours at :3000 is neither reused nor overwritten. A test server that is already running at :3100 is reused. The suite takes a few minutes.

`npm run test:local` runs the same suite against local mode. It always starts its own API, on a fresh `WEAVELING_DATA` directory under `target/web/local-data/`. So it needs port 3100 to be free. It must not reuse an in-memory server, because that would test the wrong thing. Local mode is the honest one about timing. Its outbox is polled, so a catalog lags a moment behind a write. Anything in the client that only worked because answers came back instantly fails here first.

**Editing a migration needs a nudge.** `sqlx::migrate!` bakes the SQL into the binary at compile time. On stable Rust, it cannot tell cargo that the `.sql` files are build inputs. So changing only a `.sql` file may not trigger a rebuild, and the binary keeps the old schema. Touch any `.rs` file in the crate, or run `cargo clean -p weaveling-eventsourcing`. A test checks that the expected indexes exist. So if a migration failed to land, the suite fails, and the problem does not go unnoticed.

Each feature keeps its data in [a database of its own](./ARCHITECTURE.md#dependency-rules). So a database test gets one throwaway PostgreSQL schema per feature. The schema is created on the spot and dropped afterwards. The suite still runs in parallel, and no test can see another test's rows. Sometimes a killed test leaves a schema behind. An hour later, whichever test runs next deletes it.

Two things to know before you write more of these tests:

- Selectors use only **role plus accessible name**, never a CSS class or an index. So restyling cannot break a test.
- The API runs in memory and is shared for the whole run. So no test may assume an empty list. Each test creates its own project with a unique name.

### Common commands

| Command | Does |
|---|---|
| `cargo build` | Builds the server-side crates. The web client is excluded from `default-members`, because cargo-leptos builds it, for a different target. |
| `cargo nextest run` | Runs the workspace test suite: every test of every crate at once ([cargo-nextest](https://nexte.st), `cargo install --locked cargo-nextest`). None of these tests needs a database. `cargo test` still works, but it is slower, because it runs one test binary after another. |
| `cargo nextest run --workspace --all-features` | Adds every test that talks to a real PostgreSQL. Needs `docker compose up -d`. That PostgreSQL accepts 300 connections, because the tests run many at once. `.config/nextest.toml` lets only two of the API's journeys on PostgreSQL run at the same time. This also keeps CI under its limit of 100 connections. |
| `cargo fmt --all` | Formats everything. |
| `cargo clippy --workspace --all-targets` | Lints the server side. **Does not cover the client**, because `--workspace` does not build for `wasm32`. |
| `cargo clippy -p weaveling-client-web --target wasm32-unknown-unknown` | Lints the client. It is needed as a separate command, because of the row above. |
| — | CI runs all of the above plus the browser suite, against a PostgreSQL service container. See [.github/workflows/check.yml](./.github/workflows/check.yml). |
| `cargo check -p weaveling-client-web --target wasm32-unknown-unknown` | Type-checks the client without building it. |
| `cargo build-release` | Produces the optimised client, split by view, in `target/site`, and the server next to it (`cargo leptos build --release --split`). |
| `cd clients/web && npm test` | Runs the client's end-to-end tests in a real browser. Starts the server itself if it is not already running, and reuses it if it is. |

### Layout

A Cargo workspace with `clients/`, `services/`, `features/` and `libraries/`. Each feature is an onion: a `core` without dependencies, wrapped in a ring of adapter crates. Each feature also has a WASM-safe `contract` crate, which is shared with the client. The rules for what may depend on what are in [ARCHITECTURE.md](./ARCHITECTURE.md#repository-structure).

One crate in `libraries/` ships with nothing: `weaveling-test-harness`. It is a dev-dependency wherever it appears, and it is never linked into a binary. It gives a test a `PostgresFixture`, which hands out a throwaway database namespace per feature. It is the only crate there that exists only for the test suite.

## Status

Early days — this document captures the dream. The solution is still being woven.
