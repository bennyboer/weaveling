# Weaveling — Architecture

This document is the *how*. For the *what* and the *why*, see [README.md](./README.md).

Nothing here is built yet — this captures the architectural decisions we've settled on so they aren't lost. Where a decision is provisional, it says so.

## Shape

Weaveling is a **client–server application**.

- **Server:** a **modular monolith** written in **Rust**.
- **Client:** a browser. "Write from anywhere with just a browser" is a core goal.
- **Database:** **PostgreSQL**.

The server is a monolith *for now*, but organised into clean modules so that seams can later become process/service boundaries without a rewrite.

## Repository Structure

A Cargo workspace with four top-level folders. Plural names throughout: each is expected to hold more than one thing eventually, even where it holds exactly one today.

```
weaveling/
├── Cargo.toml      [workspace] + [workspace.dependencies]
├── clients/        one crate per client (web first)
├── services/       deployable units — exactly one for now
├── features/       vertical slices of the domain
└── libraries/      cross-cutting, domain-free technique
```

**Why many crates:** in Rust a crate boundary *is* an enforced dependency rule. "Persistence must not leak into business logic" stops being a code-review convention and becomes a compile error, because the crate simply doesn't list the dependency. Crates also give parallel compilation and caching.

**Aim for a wide, shallow graph.** Crate count alone doesn't shorten builds — a crate is the unit of both parallelism *and* recompilation, so a deep chain rebuilds serially all the way up. Libraries should be stable leaves; depth is the enemy.

### Feature anatomy — the onion

Each feature is a slice of the domain (project management, structure tree, timeline, …) and is internally an **onion**: a pure core, wrapped in a ring of adapters.

```
features/
└── projects/
    ├── contract/          weaveling-projects-contract   → —      (shared kernel, WASM-safe)
    ├── core/              weaveling-projects-core       → —      (domain, ports, ProjectService)
    └── adapters/
        ├── rest/          weaveling-projects-rest       → core, contract   (inbound)
        └── store/         weaveling-projects-store      → core             (outbound)
```

```
     contract         core
         ↑           ↑    ↑
         └── rest ───┘    └── store
               ↑              ↑
               └─── service ──┘
```

- **`core`** — domain types, business logic, the **ports** (traits) it needs, and the application facade (`ProjectService`). Depends on nothing. Plain Rust, no frameworks, no magic.
- **`adapters/*`** — **one crate per adapter**, named for the foreign system it talks to. All arrows point inward at `core`.
  - *Outbound (driven)* adapters implement a port declared by core. Named after the port: `ProjectStore` → `store`.
  - *Inbound (driving)* adapters call core's public API. Named after the transport: `rest`, later `graphql`, `cli`, `messaging`. No inbound port trait — `ProjectService` is already the interface.
- **`contract`** — the wire types. Deliberately **not** part of the onion: it is a shared kernel between two *processes*, and it exists as its own crate for a hard technical reason — the WASM client cannot depend on `rest`, because `axum` doesn't compile to WASM. The constraint is **`serde` and nothing else**. Values travel in their primitive wire representations — ids and timestamps as strings — while the rich domain types (`ProjectId`, time types) stay in `core` and the adapter maps between them. This keeps the crate trivially WASM-safe and gives the domain vocabulary exactly one owner. Note that ids arriving *inbound* come through the URL path, which `rest` parses with its own extractor, so contract is almost entirely an outbound-shape concern.

There is deliberately **no `boundary` crate**. Once every adapter has its own crate, "boundary" names whatever is left over — which is nothing. Mapping belongs to the adapter that owns the DTOs, the facade lives in core, and wiring is the service's job.

**When *not* to split an adapter out:** the test is *different dependencies, or a different swap story?* REST vs. store: yes to both — split. REST vs. a hypothetical GraphQL adapter over identical DTOs: no to both — one crate. Split on dependency weight and swappability, not on conceptual tidiness.

**Adapters are siblings, never friends.** `rest` must not depend on `store`. Choosing the concrete persistence implementation is the **service's** job as composition root — that is what keeps the in-memory → PostgreSQL swap down to one line in one manifest.

### Dependency rules

| Layer | May depend on | Never |
|---|---|---|
| `clients` | contract crates | features, services |
| `services` | features, libraries | clients |
| `features` | libraries, own contract | **other features** |
| `libraries` | libraries (shallowly) | features, services |

The **feature → feature** ban is the load-bearing one. When two features want the same thing there are exactly two legal moves: the *service* composes them, or the shared concept sinks into a `libraries/` crate. (Expected first case: structure tree and timeline sharing a data structure.)

### Naming and conventions

- **Prefix every crate.** Cargo crate names are flat and global; directories only group. `weaveling-projects-core`, not `core`.
- **Alias dependencies to stay readable.** `projects-core = { package = "weaveling-projects-core", path = "../core" }` gives `use projects_core::Project;` at the call site. Set this up from the first manifest — retrofitting means touching every import.
- **Shared versions in `[workspace.dependencies]`**, with member crates writing `serde = { workspace = true }`. One place to bump, and it prevents split-version build explosions. Glob members let new crates auto-join, but the patterns must match the crate depth exactly (`"features/*/contract"`, `"features/*/core"`, `"features/*/adapters/*"`) — a greedy `**` also matches intermediate directories that hold no `Cargo.toml`.
- **`features/` collides with Cargo features** (`[features]`, `--features`, `#[cfg(feature = "…")]`). Accepted knowingly: legibility to any programmer beat avoiding the clash. `modules/` collides with `mod` and `domains/` is more jargon, so there was no clean winner.

### Consequences in Rust

Three practical decisions that follow from the layout:

- **Ports use `Arc<dyn Trait>`, not generics.** A `<S: ProjectStore>` type parameter is infectious — it propagates into every caller and fights axum's `State` extractor. The vtable lookup is noise next to I/O.
- **Async traits need `async-trait`.** `async fn` in traits is stable but *not* dyn-compatible, so `Arc<dyn ProjectStore>` requires the boxing that `#[async_trait]` provides.
- **DTO mapping is free functions, not `From` impls.** The orphan rule forbids `impl From<Project> for ProjectResponse` inside `rest`, since neither type is local to it. Plain `fn to_response(p: &Project) -> ProjectResponse` has no such restriction and keeps the arrows correct. Making `core` depend on `contract` to get the impls would leak the wire format into the domain.

## Identifiers

**UUID v7** ([RFC 9562](https://www.rfc-editor.org/rfc/rfc9562), 2024) everywhere — not v4. A 48-bit Unix-millisecond prefix followed by 74 random bits, so ids are lexicographically sortable and sort order equals creation order.

- **Why:** monotonic keys append at the right edge of a B-tree instead of scattering inserts across it — fewer page splits, less dirtied cache, less WAL. Postgres uses heap tables rather than a clustered index, so the effect is milder than the MySQL horror stories, but the primary-key index still pays. This is irrelevant for projects (dozens per user) and decisive for **nodes** (thousands per book) and the **event log** (append-only, the most insert-heavy table in the whole design). Setting one convention now beats choosing per entity later. Bonus: `ORDER BY id` *is* creation order, so cursor pagination needs no extra column.
- **Cost:** an id leaks its creation time to millisecond precision, and entropy drops from 122 random bits to 74 (still far past any collision concern). Acceptable for a login-gated tool; revisit if ids ever land in public share URLs.
- **The timestamp is injected, never read inside the domain.** Use `Uuid::new_v7(Timestamp::from_unix(…))` fed by the same `now` the aggregate receives — not `Uuid::now_v7()`, which would smuggle a clock read into `core`. This keeps the domain pure, keeps tests deterministic, and makes an id's embedded timestamp agree with its aggregate's `created_at` by construction rather than by luck.

## The Two-Speed Model

The single most important architectural decision. The domain splits into two regimes with very different characteristics, and each gets the tool that fits it.

### 1. Structure — Event Sourcing + CQRS

Coarse-grained, relatively low-frequency changes where history has real value:

- create / move / split / reorder nodes
- attach a codex entity to a node
- add or route a thread
- set time buckets and before/after relations

This regime uses **event-sourcing + CQRS**. It buys us undo/redo, time-travel, a full audit ("who changed what"), and a natural basis for merging structural changes between collaborators.

### 2. Prose — CRDT

The actual typing inside a node is character-level, high-frequency, and needs **intention-preserving merge** when multiple authors edit the same paragraph at once. Event-sourcing does **not** solve this on its own, and routing keystrokes through command validation + projections would make typing laggy.

Prose therefore uses a **CRDT**:

- **`yrs`** (the Rust port of Yjs / y-crdt) on the server,
- **Yjs** on the client, bound to the editor.

CRDT updates *may* still be persisted into the durable log, but they never pass through command-validation or a broker.

### Presence is neither

Cursor position, selection, and who's-online are **ephemeral awareness** state. They are **never** event-sourced and **never** persisted — they travel over the WebSocket (Yjs awareness protocol) and are discarded.

## Local-First

Because prose is a CRDT, the client keeps a **full replica in the browser (IndexedDB)**. This gives us, almost for free:

- **offline editing** — the server being down or unreachable never blocks writing,
- **zero typing latency** — edits apply locally and sync in the background,
- a concrete expression of the "your data is yours" value.

The client is designed to be **local-first-capable**, not a thin dumb terminal.

## Event Sourcing — Discipline

- **Be selective.** Event-source only where history has value (the structural domain, undo/redo). Plain state for settings, presence, search indexes, and blobs. "ES where history has value" — not everything.
- **Event store:** rolled on **PostgreSQL** — an events table with per-aggregate streams and optimistic concurrency via a version column.
- **Projections / async work:** a **transactional outbox** + poller for reliable projection updates; PostgreSQL `LISTEN/NOTIFY` (or logical replication) to nudge the WebSocket gateway. No message broker for now (see below).
- **Versioning / upcasting** is planned from day one — a book project lives for years and events will evolve.
- **Snapshots** for aggregates to keep rebuilds fast at scale (a paragraph-per-node book is thousands of nodes).

## Messaging Broker — Deferred (option kept open)

We start **without** a broker (no RabbitMQ/AMQP yet). Inside a modular monolith, in-process dispatch + the Postgres-backed outbox goes a long way and avoids dual-write/ordering complexity before it pays off.

A broker can be introduced later at a real module boundary — e.g. when there are separately-deployed consumers or cross-service async handoff — **without rearchitecting**, provided the module seams stay clean. Keeping this option open is an explicit goal.

## Frontend — Full-Stack Rust (provisional)

We try the **full-stack Rust** path first (e.g. Leptos / Dioxus / Yew compiled to WASM). It's part of the fun and keeps the whole stack in one language.

**Fallback:** if building a collaborative rich-text editor with strong typography proves too heavy in Rust/WASM, we switch the frontend to something like **Angular** (with a proven editor + Yjs bindings), keeping the Rust backend. This is a known, acceptable escape hatch — not a failure.

## Export

Two distinct export pipelines, driven by an in-order walk of the structure:

- **PDF** via **Typst** (Rust-native typesetting, embedded as a library — implement its `World` trait for font/file resolution). Print-quality output.
- **EPUB** via a separate path: structure walk → semantic HTML → EPUB package (reflowable XHTML+CSS). Reuses the web view's HTML rendering. Typst does **not** produce EPUB.

Font bundling/licensing for embedded fonts is a concern for both.

## Supporting Concerns (noted for later)

- **Search & mention detection** — projections built with **Tantivy** (Rust-native, Lucene-like): full-text search over prose, plus codex alias/mention indexing (the "which nodes does this character appear in" back-links).
- **Blobs** (codex and research images) — object storage or filesystem, referenced by ID from events. Not stored as byte columns and not in the event log.
- **Multi-author** — implies accounts, project membership, and permissions. Real scope, acknowledged early.

## Status

Design captured; nothing implemented. The solution is still being woven.
