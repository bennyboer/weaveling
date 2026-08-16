# Weaveling — Roadmap

The sequenced plan. For the *what/why* see [README.md](./README.md), for the *how* see [ARCHITECTURE.md](./ARCHITECTURE.md), for loose notes see [TODO.md](./TODO.md).

This roadmap covers **Phase 1** in detail and only sketches what comes after. Phase 1 is deliberately small: a working client–server loop where you can create, rename, list and delete **projects** — nothing else. No nodes, no prose, no event sourcing, no database.

## How we work

The point of Phase 1 is as much *getting fluent in Rust again* as it is shipping the prototype.

- **You write the code.** All of it, unless you explicitly hand a piece over.
- **I teach.** Before each step I explain the concepts involved, sketch the shape (types, signatures, module layout) and point at the docs worth reading. I do not fill in the bodies.
- **I review.** After each step I read what you wrote and give feedback: correctness first, then idiom — the "a Rust dev would write this differently, and here's why" pass.
- **When you're stuck**, ask and I'll explain rather than just patch it.

## Guiding constraints for Phase 1

- **In-memory store only.** A `HashMap` behind a lock. Restarting the server loses everything — that's fine for now.
- **Persistence is abstract from day one.** All storage goes through a trait. Swapping in Postgres later must mean writing one new impl and changing one line of wiring — nothing else.
- **No event sourcing yet.** Plain CRUD on plain state. The two-speed model is real, but a project's name is exactly the kind of thing that stays plain state anyway.
- **Vertical slice.** Get one thin feature working end to end (browser → HTTP → store → back) before making anything wide.

---

## Milestone 0 — Scaffold ✅

**Goal:** `cargo run` starts a server that answers on a health endpoint.

**Build:**
- Cargo **workspace** following the layout settled in [ARCHITECTURE.md](./ARCHITECTURE.md#repository-structure) — `clients/`, `services/`, `features/`, `libraries/`, with the `projects` feature split into `contract`, `core` and `adapters/{rest,store}`.
- `[workspace.dependencies]` at the root, dependency aliasing in member manifests.
- Pick the async runtime + web framework. Recommended: **tokio** + **axum**.
- A `GET /health` returning 200 from `services/api`.

**Rust you'll meet:** workspace layout, `Cargo.toml` and workspace dependencies, crates vs. modules, `mod`/`pub`/`use`, the `async fn main` + `#[tokio::main]` entry point.

Empty crates that just compile are a fine M0 deliverable — the point is the skeleton and the dependency arrows, not behaviour.

**Done when:** the server starts and `curl localhost:PORT/health` answers.

*Done. Six crates, `cargo build` green, `/api/health` returns 200, `trunk serve` builds and serves the WASM client, and the dev proxy reaches the API same-origin. See the README for how to run it.*

## Milestone 1 — The domain: `Project` ✅

**Goal:** a `Project` type you're happy with, plus tests.

**Build:**
- `Project` — id, name, timestamps. Consider a newtype `ProjectId` over `Uuid` rather than a bare `Uuid`.
- Validation: what makes a name valid? Empty names rejected, trimmed, length bounds.
- Unit tests for the validation rules.

**Rust you'll meet:** structs and `impl` blocks, derive macros (`Debug`, `Clone`, `PartialEq`), the **newtype pattern**, `Option` vs. `Result`, constructing fallible values, `#[cfg(test)] mod tests`.

**Done when:** `cargo test` passes and creating an invalid project is impossible by construction.

*Done. `ProjectId`, `ProjectName` and `Project` in `features/projects/core`, 20 tests. Validity holds by construction because `Project` stores a `ProjectName`, never a `String`. Wall-clock time is **injected** (`Project::new(name, now)`) rather than read inside the domain — the application service owns the clock, and tests stay deterministic. Ids are sortable UUID v7 built from that same `now`; see [ARCHITECTURE.md](./ARCHITECTURE.md#identifiers).*

## Milestone 2 — The store trait + in-memory impl ✅

**Goal:** the abstraction that makes Postgres a later, cheap decision. The heart of Phase 1.

**Build:**
- A `ProjectStore` trait: create, get, list, rename (or a general update), delete.
- Its error type — a `StoreError` enum (`NotFound`, `Conflict`, `Backend`), deliberately *not* leaking any storage technology into the signature.
- An `InMemoryProjectStore` implementing it, holding a `HashMap<ProjectId, Project>`.
- Tests written **against the trait**, so the same suite can later validate the Postgres backend. The suite lives in the store adapter crate as a `#[cfg(test)]` module, since every backend of a port shares that crate.

**Crate placement:** the `ProjectStore` trait belongs in `core` — a port is declared by the domain that needs it. The `HashMap` impl goes in `adapters/store`. Nothing above the trait knows which impl exists; the service picks.

**Rust you'll meet:** traits and trait bounds, `async` in traits, **shared mutable state** (`Arc` + `RwLock`/`Mutex`, interior mutability), static vs. dynamic dispatch (generics vs. `dyn Trait`), error modelling with `thiserror`, `?` and `From` conversions.

**This is where the design lives.** Expect to spend real time here and expect me to push back on the trait shape — if it's leaky, everything downstream inherits the leak.

**Done when:** the trait test suite passes against the in-memory impl, and nothing above the trait knows what's behind it.

*Done. `ProjectStore` + `StoreError` in `core`; `adapters/store` holds `memory.rs` (the backend) and `suite.rs` (9 `#[cfg(test)]` conformance cases over `&impl ProjectStore`). Postgres will be a second **module in the same crate** behind an optional feature, not a sibling crate — so it reuses the suite directly and `core` needs no test scaffolding. `list` promises id order, which is creation order thanks to v7. Two known gaps, both fine for a single-user MVP: read-modify-write has no optimistic concurrency, so concurrent renames can lose an update (a version column is the fix); and the store is **single-tenant** — `list` returns every project and `get` will serve any id to anyone, which stops being acceptable the moment accounts exist. See [ARCHITECTURE.md](./ARCHITECTURE.md#supporting-concerns-noted-for-later).*

## Milestone 3 — HTTP API ✅

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

## Milestone 4 — Frontend ✅

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

## Milestone 5 — Tidy up (partly deferred)

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

---

## After Phase 1 (sketch only)

Not planned in detail — we'll sequence these once Phase 1 lands and we know how the code actually feels.

- **Nodes and the structure tree** — the real domain. Where event sourcing starts to earn its place.
- **Prose + the yrs ↔ Yjs round-trip** — was the highest-risk assumption in the whole design. **De-risked**, one small rung at a time: CRDT semantics ✅, `yrs` ↔ `Yjs` wire compatibility including insertion ties ✅, reading and re-writing a `y-prosemirror` document from Rust ✅, Leptos hosting ProseMirror with two replicas converging across a simulated partition ✅, and real `y-websocket` clients syncing through our own Rust server ✅. Three spikes — `spikes/crdt`, `spikes/editor`, `spikes/sync` — and the findings are recorded in [ARCHITECTURE.md](./ARCHITECTURE.md#prose--the-editing-stack). What remains is implementation, not discovery: persistence, compaction, auth on the socket, and joining it to the structure tree.
- **Postgres** — write the second `ProjectStore` impl and prove the abstraction was worth it.
- Then, in some order: codex, timeline, threads, export, accounts.

## Explicitly not in Phase 1

Postgres · event sourcing · CRDTs · WebSockets · auth · the codex · the timeline · threads · export · offline support.
