# Weaveling — Roadmap

The sequenced plan. For the *what/why* see [README.md](./README.md), for the *how* see [ARCHITECTURE.md](./ARCHITECTURE.md), for loose notes see [TODO.md](./TODO.md).

**Phase 1** (a projects-only CRUD prototype) and **Phase 2** (spiking the prose stack) are done. **Phase 3** — turning the spikes into the product — is planned below in detail; everything after it is a sketch.

## How we work

The point of Phase 1 is as much *getting fluent in Rust again* as it is shipping the prototype.

- **You write the code.** All of it, unless you explicitly hand a piece over.
- **I teach.** Before each step I explain the concepts involved, sketch the shape (types, signatures, module layout) and point at the docs worth reading. I do not fill in the bodies.
- **I review.** After each step I read what you wrote and give feedback: correctness first, then idiom — the "a Rust dev would write this differently, and here's why" pass.
- **When you're stuck**, ask and I'll explain rather than just patch it.

## Phase 1 — A projects-only prototype

Deliberately small: a working client–server loop where you can create, rename, list and delete **projects** — nothing else. No pieces, no prose, no event sourcing, no database.

### Guiding constraints for Phase 1

- **In-memory store only.** A `HashMap` behind a lock. Restarting the server loses everything — that's fine for now.
- **Persistence is abstract from day one.** All storage goes through a trait. Swapping in Postgres later must mean writing one new impl and changing one line of wiring — nothing else.
- **No event sourcing yet.** Plain CRUD on plain state. The two-speed model is real, but a project's name is exactly the kind of thing that stays plain state anyway.
- **Vertical slice.** Get one thin feature working end to end (browser → HTTP → store → back) before making anything wide.

---

### Milestone 0 — Scaffold ✅

**Goal:** `cargo run` starts a server that answers on a health endpoint.

**Build:**
- Cargo **workspace** following the layout settled in [ARCHITECTURE.md](./ARCHITECTURE.md#repository-structure) — `clients/`, `services/`, `features/`, `libraries/`, with the `projects` feature split into `contract`, `core` and `adapters/{rest,store}`.
- `[workspace.dependencies]` at the root, dependency aliasing in member manifests.
- Pick the async runtime + web framework. Recommended: **tokio** + **axum**.
- A `GET /health` returning 200 from `services/api`.

**Rust you'll meet:** workspace layout, `Cargo.toml` and workspace dependencies, crates vs. modules, `mod`/`pub`/`use`, the `async fn main` + `#[tokio::main]` entry point.

Empty crates that just compile are a fine M0 deliverable — the point is the skeleton and the dependency arrows, not behavior.

**Done when:** the server starts and `curl localhost:PORT/health` answers.

*Done. Six crates, `cargo build` green, `/api/health` returns 200, `trunk serve` builds and serves the WASM client, and the dev proxy reaches the API same-origin. See the README for how to run it.*

### Milestone 1 — The domain: `Project` ✅

**Goal:** a `Project` type you're happy with, plus tests.

**Build:**
- `Project` — id, name, timestamps. Consider a newtype `ProjectId` over `Uuid` rather than a bare `Uuid`.
- Validation: what makes a name valid? Empty names rejected, trimmed, length bounds.
- Unit tests for the validation rules.

**Rust you'll meet:** structs and `impl` blocks, derive macros (`Debug`, `Clone`, `PartialEq`), the **newtype pattern**, `Option` vs. `Result`, constructing fallible values, `#[cfg(test)] mod tests`.

**Done when:** `cargo test` passes and creating an invalid project is impossible by construction.

*Done. `ProjectId`, `ProjectName` and `Project` in `features/projects/core`, 20 tests. Validity holds by construction because `Project` stores a `ProjectName`, never a `String`. Wall-clock time is **injected** (`Project::new(name, now)`) rather than read inside the domain — the application service owns the clock, and tests stay deterministic. Ids are sortable UUID v7 built from that same `now`; see [ARCHITECTURE.md](./ARCHITECTURE.md#identifiers).*

### Milestone 2 — The store trait + in-memory impl ✅

**Goal:** the abstraction that makes Postgres a later, cheap decision. The heart of Phase 1.

**Build:**
- A `ProjectStore` trait: create, get, list, rename (or a general update), delete.
- Its error type — a `StoreError` enum (`NotFound`, `Conflict`, `Backend`), deliberately *not* leaking any storage technology into the signature.
- An `InMemoryProjectStore` implementing it, holding a `HashMap<ProjectId, Project>`.
- Tests written **against the trait**, so the same suite can later validate the Postgres backend. The suite lives in the store adapter crate as a `#[cfg(test)]` module, since every backend of a port shares that crate.

**Crate placement:** the `ProjectStore` trait belongs in `core` — a port is declared by the domain that needs it. The `HashMap` impl goes in `adapters/store`. Nothing above the trait knows which impl exists; the service picks.

**Rust you'll meet:** traits and trait bounds, `async` in traits, **shared mutable state** (`Arc` + `RwLock`/`Mutex`, interior mutability), static vs. dynamic dispatch (generics vs. `dyn Trait`), error modeling with `thiserror`, `?` and `From` conversions.

**This is where the design lives.** Expect to spend real time here and expect me to push back on the trait shape — if it's leaky, everything downstream inherits the leak.

**Done when:** the trait test suite passes against the in-memory impl, and nothing above the trait knows what's behind it.

*Done. `ProjectStore` + `StoreError` in `core`; `adapters/store` holds `memory.rs` (the backend) and `suite.rs` (9 `#[cfg(test)]` conformance cases over `&impl ProjectStore`). Postgres will be a second **module in the same crate** behind an optional feature, not a sibling crate — so it reuses the suite directly and `core` needs no test scaffolding. `list` promises id order, which is creation order thanks to v7. Two known gaps, both fine for a single-user MVP: read-modify-write has no optimistic concurrency, so concurrent renames can lose an update (a version column is the fix); and the store is **single-tenant** — `list` returns every project and `get` will serve any id to anyone, which stops being acceptable the moment accounts exist. See [ARCHITECTURE.md](./ARCHITECTURE.md#supporting-concerns-noted-for-later).*

### Milestone 3 — HTTP API ✅

**Goal:** projects are fully manageable over REST.

**Build:**
- `ProjectService` in `core` — the facade. Takes **primitives** (`&str` for both ids and names) and turns them into domain types, so validation is a business rule every adapter inherits rather than a transport concern. Owns the clock via `libraries/clock`. ✅
- Request/response DTOs in `contract`, ids and timestamps as strings. ✅
- `GET /projects`, `POST /projects`, `GET /projects/{id}`, `PATCH /projects/{id}`, `DELETE /projects/{id}`.
- Store injected as shared application state.
- Errors mapped to sensible status codes (404, 400, 409).

**Rust you'll meet:** axum routing, handlers, extractors (`Path`, `Json`, `State`), **serde** derive and attributes, implementing `IntoResponse` for your error type, the async/await model and why the store needs `Send + Sync`.

**Done when:** the full lifecycle works via `curl`, and errors return the right codes rather than 500s.

*Done. Five routes under `/api/projects`, 60 tests. `services/api` is now **lib + thin bin** so `fn app(store, clock)` is testable — the bin only picks the backend and serves. `rest` wraps `ProjectError` in a local `ApiError` newtype because the orphan rule forbids `impl IntoResponse for ProjectError`; that newtype owns the status mapping (400 / 404 / 409) and deliberately **does not** leak `StoreError::Backend` detail to clients — it logs the cause and answers a generic 500. Verified over real HTTP with curl: create trims the name, blank name → 400, malformed id → 400, unknown id → 404, delete → 204 then 404.*

### Milestone 4 — Frontend ✅

**Goal:** a browser UI listing projects, with create / rename / delete.

**Decision needed before starting:** which full-stack Rust framework. **Recommendation: Leptos** — the largest ecosystem of the three, signal-based reactivity that will feel familiar, and the best documentation for someone getting back into Rust. Dioxus and Yew stay open; nothing in M0–M3 depends on the choice.

**Build:**
- Project list view, plus create / rename / delete.
- Talk to the API over HTTP; handle loading and error states honestly.
- Sort out CORS or same-origin serving.

**Rust you'll meet:** the WASM target and toolchain, components and signals, async data fetching from the browser, and the reality of debugging Rust in a browser.

**Done when:** the whole loop works in a browser with no `curl` involved.

*Done, and **verified in a real browser** — create (with trimming), rename, delete, the error banner, and Enter-to-submit all exercised end to end. `clients/web` has `api.rs` (gloo-net against `/api/projects`, reusing the `contract` DTOs) and `app.rs` (`App` + `ProjectRow`, explicit loading / empty / error states, inline rename via an edit toggle).*

*Two bugs the browser found that no test could have:*
- *Every mutation set `problem` then called `reload()` unconditionally — and `reload()` clears `problem` on success, so error banners were wiped microseconds after being set. Mutations now reload **only on success**.*
- *`<form on:submit>` with `event.prevent_default()` did not prevent the native submit, so the page reloaded and reset every signal mid-request. Replaced with a plain `<div>` plus an explicit click handler and an Enter `on:keydown` — no form semantics were needed.*

***Timestamp formatting belongs to the client.*** *`contract` carries RFC 3339 (machine-readable, unambiguous); the client parses it and renders `14 Aug 2026, 11:40` in the **viewer's** timezone via `time`'s `local-offset` + `wasm-bindgen` features. Presentation and locale are the viewer's business, not the wire's.*

*Rename and delete are rare actions, so they live behind a **kebab menu** per row rather than cluttering every line with two buttons. Only one menu opens at a time (a single `open_menu` signal in `App`), and a window-level `click` / `keydown` listener closes it on outside-click or Escape.*

***Delete asks first.*** *An in-app modal names the project and states that it cannot be undone; Cancel, Escape and backdrop-click all dismiss it. Deliberately **not** `window.confirm()` — that blocks the event loop, cannot be styled, and makes the app untestable through browser automation. Known gap: the modal has no focus trap, so keyboard users can tab behind it. Worth fixing with `<dialog>` + `showModal()`, which gives trapping and Escape natively.*

*One unexplained observation: a single `DELETE` showed 503 in the browser network panel while the delete itself succeeded. Not reproducible — `curl` returns 204 both through the Trunk proxy and direct. Recorded rather than papered over.*

**Escape hatch:** if the editor/typography story later proves too painful in Rust/WASM, the frontend switches to Angular against the same API. Phase 1 is small enough that finding this out here is cheap — that's part of the point.

### Milestone 5 — Tidy up (partly deferred)

Originally: structured logging, config via environment, a README section on running it locally, and CI.

**Only the bundle measurement was done.** The rest — CI, `ServeDir`, logging and env config — is deployment plumbing, and there is nothing worth deploying yet. Deferred to [TODO.md](./TODO.md) rather than done speculatively. The README section is written.

Known gaps, found while building M3/M4:

- **`TraceLayer` currently logs nothing.** It emits at DEBUG but `tracing_subscriber::fmt::init()` defaults to INFO, so there is no request log at all — which made debugging the client harder than it needed to be. Needs a sensible default filter (e.g. `RUST_LOG` with a fallback of `info,tower_http=debug`).
- **CI must run `--all-features`**, or feature-gated code (the future Postgres backend) will never be type-checked.
- **Client lint needs its own step**: `cargo clippy --workspace` does not cover `wasm32`, so the client needs `cargo clippy -p weaveling-client-web --target wasm32-unknown-unknown` explicitly.
- **Measure the release bundle.** ✅ Done:

  | | wasm | js |
  |---|---|---|
  | debug | 5347 KB | 39 KB |
  | `trunk build --release` | **401 KB** | 37 KB |
  | release, gzipped | **151 KB** | 7 KB |

  **~158 KB over the wire**, a 13× reduction from debug. That is competitive with a modest JavaScript SPA, which settles the "is full-stack Rust viable in the browser" worry from ARCHITECTURE's provisional framing. Worth re-measuring once the editor and CRDT land, since `yrs` will not be free.
- **Serve the client from the API (`ServeDir`).** `services/api` currently mounts only `/api`, so outside `trunk serve` there is nothing serving the app. The single-origin story the client depends on — relative `/api/...` URLs, hence no CORS, no build-time host config, no mixed-content trap — is real in development only because Trunk proxies. Making it true in production means a `tower_http::services::ServeDir` fallback over `clients/web/dist`, with an index fallback so client-side routes still resolve. The `fs` feature is already enabled on `tower-http` in anticipation.

### Explicitly not in Phase 1

Postgres · event sourcing · CRDTs · WebSockets · auth · the codex · the timeline · threads · export · offline support.

---

## Phase 2 — De-risk the prose stack ✅

Not application code: five rungs of spike, each one answering a question that could have forced a rewrite. All green.

| Rung | Question | Answer |
|---|---|---|
| 1 | Do CRDTs behave the way we think? | Yes — YATA semantics, tombstones, state vectors |
| 2 | Is `yrs` really wire-compatible with `Yjs`? | Yes, including same-gap insertion ties |
| 2.5 | Can the server read prose out of a rich document? | Yes — byte-identical to ProseMirror's `textBetween` |
| 4 | Can Leptos host ProseMirror without misery? | Yes — a ~20-line interop boundary |
| 3 | Can Rust be the sync server? | Yes — real `y-websocket` clients sync through it |

Three spikes — `spikes/crdt`, `spikes/editor`, `spikes/sync` — kept out of the workspace's default build. Every decision they settled is written up in [ARCHITECTURE.md](./ARCHITECTURE.md#prose--the-editing-stack); every gap they exposed is in [TODO.md](./TODO.md). What remains is implementation, not discovery.

**Read the spikes as proof, not as a starting point.** They are proof-of-concept code: no ports, no error modeling, no persistence. Phase 3 rewrites them into the architecture rather than moving them.

---

## Phase 3 — Making it real

Turning the spikes into the product. The order below is deliberate and the first milestone is the non-obvious one.

### Milestone 6 — The walking skeleton

**Goal:** one project, **one passage with nothing above it**, real collaborative prose, end to end.

The instinct is to build the structure first and attach prose later. Don't. Integration risk is highest right now while the spike knowledge is hot, and a passage standing alone is by far the cheapest place to discover that (say) the room registry belongs somewhere we didn't expect. This milestone exists to force the architecture questions into the open against the smallest possible domain.

**Build:**
- `features/passages/` in full — `contract`, `core`, `adapters/{store,sync}`, `tests` — following the anatomy settled in [ARCHITECTURE.md](./ARCHITECTURE.md#the-passages-feature).
- `Passage` wrapping a `yrs::Doc`, the plain-text projection lifted out of `spikes/crdt`, and the `PassageStore` port.
- An in-memory `PassageStore` plus a **conformance suite**, exactly as `ProjectStore` got one — the suite is what makes Postgres cheap later.
- `adapters/sync`: the y-protocols codec and room registry rewritten from `spikes/sync`, mounted at `/sync/{passage}`.
- The client editor from `spikes/editor`, pointed at the real server through `y-websocket` instead of the hand-rolled relay. **The client must never compute a passage id** — it asks what to open and gets an opaque one back, which is what keeps the id scheme a server-side detail.

**Decision made:** the port does **not** choose between snapshots and an update log. `absorb(id, update)` takes a delta, and a full snapshot is just a delta from nothing, so a backend can merge-on-write or append-and-compact without the port caring. The conformance suite tests semantics — what you absorbed, you can load — not storage shape. The real choice moves to M9, with the tiebreakers already gathered: the log is 11–13× the compacted form after 500 rewrites, but a snapshot backend needs row locking for concurrent `absorb` while an append-only one does not. See [Transactions and Atomicity](./ARCHITECTURE.md#transactions-and-atomicity).

**Rust you'll meet:** axum WebSocket handlers, `tokio` `broadcast`/`mpsc` and task lifetimes, `wasm-bindgen` ES-module imports, Trunk build hooks (and its watch-ignore trap).

**Done when:** two browser tabs edit the same passage, converge, survive a reload, and `GET /api/passages/{id}/text` returns what both tabs show.

**Done.** All four conditions are covered by `clients/web/e2e/passages.spec.ts`: two tabs on one passage converge in both directions, prose survives a reload, and the server's own projection returns what the tabs show. One deviation from the plan above — the projection is read from `GET /api/passages/{id}`, which returns `PassageDTO { id, text }`, rather than a separate `/text` route; a passage has exactly one representation to fetch, so a second route would have earned nothing.

The open passage is carried in the URL as `?passage=<id>`, which is what makes both the reload and the second tab work at all — see [Client conventions](./ARCHITECTURE.md#client-conventions).

**Explicitly not in M6:** pieces, any arrangement of them, event sourcing, a database, auth, awareness tombstones.

### Milestone 7 — Event sourcing, for real: the pool of pieces

**Goal:** the event-sourcing machinery exists and earns its keep, proven against the smallest domain that needs it.

The design is settled in [Pieces and views](./ARCHITECTURE.md#pieces-and-views--the-non-linear-model). A piece is an id, a title and a link to its passage — it does not know where it sits. The tree that used to be milestones 7 and 8 is now one view among several, and it moved to M10.

**Build:**
- `libraries/eventsourcing` — the aggregate trait (command -> events, event -> state), an `EventStore` port with per-aggregate streams and optimistic concurrency on a version column, an in-memory backend, and a **conformance suite**, exactly as `ProjectStore` and `PassageStore` each got one.
- Event metadata carrying **agent, occurred-at and aggregate version** from the very first event written. The agent holds a placeholder until accounts exist, and cannot be backfilled later.
- **Per-event versions plus a patcher** — pure `from -> to` functions applied on read. Write one real patch, however trivial, so the mechanism is exercised rather than merely present: the earlier implementation had this exact design and never once ran it.
- **Snapshots**, since replaying thousands of pieces on every load is the failure they exist to prevent.
- `features/pieces` — `PieceCaptured`, `PieceRetitled`, `PassageAttached`, `PieceDiscarded`, with the passage attached **lazily on first write**.
- A minimal client: capture a piece, retitle it, open it in the M6 editor. A plain list — not a board.

**Reviewable steps:** the library plus its conformance suite; then the `pieces` aggregate against it; then the REST adapter; then the client list. Each stands on its own for review.

**Rust you'll meet:** trait objects for aggregates, `serde` tagging for event payloads, and the ownership question of who holds the reconstructed state.

**Done.** Capture, retitle and discard land in an event stream; a piece opened for writing is given a passage and the M6 editor works inside it; a stream rebuilds exactly, including from a snapshot alone once the events it replaced are pruned; and one event really goes through a patch — the sample aggregate carries a `CreatedBeforeKinds` variant at version 0 that the aggregate deliberately **cannot** read, so a stream written in the old shape rebuilding at all is proof the patch ran. Removing the patch fails two tests.

**Done when:** capture, retitle and discard all land in an event stream; a piece opened for writing gets a passage and the M6 editor works inside it; rebuilding from the stream reproduces state exactly; and at least one event has been through a patch.

**Explicitly not in M7:** the board, placement, messaging, the outline, any durable store.

### Milestone 8 — Messaging and the cascade

**Goal:** the seam features talk across, plus the first thing that genuinely needs it.

**Build:** `libraries/messaging` — the publish/subscribe port, the envelope (message id, **correlation and causation ids**, occurred-at, agent) and an in-process dispatcher. Then the **project-deletion saga**: deleting a project disposes its pieces, boards and passages, tolerating a project that never had a board.

**Why before the board:** the board needs a live channel pushing events to browsers, and that *is* event fan-out — a far better first consumer for the seam than the deletion saga, which stays theoretical while everything is in memory (orphans die at process restart, so a cascade cannot fail in a way anyone would notice). Built the other way round, the board would grow a bespoke fan-out that a later milestone has to reconcile. The honest caveat: `LivePassages` broadcasts CRDT frames while a board broadcasts events, so the two may share less than it first appears.

**Done when:** deleting a project leaves nothing behind; a saga that fails midway is recoverable and observable; and every message in a run traces back to the request that caused it.

**Explicitly not in M9:** a broker. Transport stays in-process — RabbitMQ is an adapter for when a second deployable exists. See [Messaging](./ARCHITECTURE.md#messaging--the-seam-now-the-transport-later).

### Milestone 9 — The board

**Goal:** an infinite corkboard — the non-linear feel that is the point of Weaveling.

**Build:** `features/boards` — `BoardStarted`, `PiecePinned`, `PieceMoved`, `PieceUnpinned`, placement owned by the board, and **find-or-start on first open**. Free 2D placement, with moves committing **on drop** and in-flight drags travelling as awareness over the board's own live channel. The client joins pool and placements itself, which is also what makes a dangling placement harmless.

**On top of the seam:** the board's socket is a subscriber rather than a bespoke broadcast, and the piece catalog's synchronous dual write becomes a projector at the same time.

**Rust you'll meet:** pointer events and transforms for a pannable infinite surface, a second WebSocket surface that is *not* a CRDT, and awareness carrying something other than a cursor.

**Done when:** two browsers drag pieces on one board and see each other live; the durable record holds one event per drop rather than per frame; a piece discarded from the pool vanishes from the board with no compensating event; and a project that never had a board gets one on first open.

**Explicitly not in M8:** the outline, grouping, board naming or switching — multiple boards are modelled, one is shipped.

### Milestone 10 — The outline

**Goal:** manuscript order, as a view over the pool rather than a property of it.

**Build:** `features/outline` — ordered nesting over pieces, acyclic, one parent per piece. Moves shaped like an author's intent (*move after*, *promote*, *demote*) rather than the "swap two nodes" primitive the earlier implementation was cornered into by its data structure.

This is the privileged view: export needs a linear order, so the outline is what "the manuscript" means. A piece may sit on the board and be absent from the outline — it simply is not in the book yet.

**Still open:** the split-piece mechanics — see the design threads in [TODO.md](./TODO.md).

**Not in M10:** undo/redo. The event stream makes it available whenever it is wanted, which is exactly why it does not need to be built alongside the outline.

**Done when:** a book-shaped outline of chapters and scenes, each openable in the editor, structural changes visible in the audit log, and rebuilding the projection from scratch reproducing the same order.

### Milestone 11 — The real store

**Goal:** prove the abstractions were worth the trouble.

**The database choice is reopened.** PostgreSQL was parked early; MongoDB is under consideration again. The decision belongs here, judged by the conformance suites rather than by preference, and it changes the outbox mechanics (`LISTEN/NOTIFY` versus change streams) without touching anything above a port.

**Build:** a second backend for every port that has one — `ProjectStore`, `PassageStore`, the event store — as modules behind an optional cargo feature, not sibling crates. CI must run `--all-features` or none of it is type-checked.

**This is where storage representation finally gets decided,** and where the transaction tests that in-memory cannot express have to be written: rollback, connection failure mapping to `StoreError::Backend`, and the concurrency guard `absorb` needs if `PassageStore` goes the snapshot route. The event store's `append` must be atomic across the version check, the append **and** the outbox insert — one transaction, invisible above the port.

**Done when:** the suites pass unchanged against the real store, and swapping backends is one line in one manifest.

---

## After Phase 3 (sketch only)

- **Accounts, tenancy and auth** — looming. It changes port shapes (`list(owner)`, not `list()`) and puts auth on the sync socket. Not a filter to bolt on.
- **Export** — Typst → PDF and HTML → EPUB. Still an **unspiked risk**: embedding Typst means implementing its `World` trait for font and file resolution, plus font licensing. Worth its own rung before it becomes a milestone.
- Then, in some order: the codex, the timeline, threads, search projections over Tantivy.
