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

**Decision made:** the port does **not** choose between snapshots and an update log. `apply(id, update)` takes a delta, and a full snapshot is just a delta from nothing, so a backend can merge-on-write or append-and-compact without the port caring. The conformance suite tests semantics — what you applied, you can load — not storage shape. The real choice moves to M9, with the tiebreakers already gathered: the log is 11–13× the compacted form after 500 rewrites, but a snapshot backend needs row locking for concurrent `apply` while an append-only one does not. See [Transactions and Atomicity](./ARCHITECTURE.md#transactions-and-atomicity).

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

### Milestone 8 — Messaging ✅

**Goal:** the seam features talk across, plus the first thing that genuinely needs it.

**Built:** `libraries/messaging` — the publish/subscribe port, the envelope (message id, **the conversation and what caused it**, occurred-at) and an in-process dispatcher. A listener names itself and declares its `Delivery` — `Kept` or `Fleeting` — which are the two things [only a feature knows](./ARCHITECTURE.md#no-exchanges-but-a-named-listener) and the exact inputs a RabbitMQ adapter later needs. **Exchanges are deliberately not modelled.** Then `libraries/eventpublishing`, which turns an aggregate's events into messages so a feature supplies only the body; and the **piece catalog as a projection**, the first real consumer, retiring the dual write.

Two things arrived that were not planned: every feature grew a [`wiring` crate](./ARCHITECTURE.md#wiring--each-feature-assembles-itself) so the composition root stopped knowing feature internals, and a [naming sweep](./ARCHITECTURE.md#client-conventions) replaced invented words with the terms the field already has.

**Done:** the catalog is fed by a listener, the listing survives redelivery and out-of-order messages, and every message traces back to the request that caused it.

**Explicitly not in M8:** a broker. Transport stays in-process — RabbitMQ is an adapter for when a second deployable exists. See [Messaging](./ARCHITECTURE.md#messaging--the-seam-now-the-transport-later).

**Deferred out of M8: the project-deletion saga.** It was scoped here and pulled, for two reasons found while building.

**It was blocked on a decision, not on effort.** `projects` was the last feature that was not event-sourced — Phase 1 CRUD over a `ProjectStore` — so there was no `project.deleted` on the wire to listen to. The choice was to event-source `projects` properly or to hand-roll a publish inside `ProjectService::delete`, which would have been a second publishing path bypassing `eventpublishing` that we would have deleted again later. [M11a step 1](#step-1--projects-becomes-an-aggregate--done) settled it the first way, and `project.deleted` is now on the wire.

**And its payoff is still theoretical.** An orphaned piece from a deleted project is unreachable — you cannot navigate to a project that no longer exists — and dies at process restart. It becomes real at [M13](#milestone-13--local-mode), where the store is a file on the author's own disk, which is also about when event-sourcing `projects` starts paying for itself: **deletion is exactly where an author wants an audit log.** The two belong together, so they now live together in [M11a](#milestone-11a--projects-event-sourced-and-the-deletion-cascade).

### Milestone 9 — The board ✅

**Goal:** an infinite corkboard — the non-linear feel that is the point of Weaveling.

**Build:** `features/boards` — `BoardStarted`, `PiecePinned`, `PieceMoved`, `PieceResized`, `PieceRaised`, `PieceUnpinned`, placement owned by the board, and **find-or-start on first open**. Free 2D placement, with moves committing **on drop**. The client joins pool and placements itself, which is also what makes a dangling placement harmless.

**On top of the seam:** the piece catalog's synchronous dual write becomes a projector at the same time.

**Rust you'll meet:** pointer events and transforms for a pannable infinite surface.

**Done when:** the durable record holds one event per drop rather than per frame; a piece discarded from the pool vanishes from the board with no compensating event; and a project that never had a board gets one on first open. **All three are met.** Two browsers seeing each other live was the fourth, and it moved out to [M9b](#milestone-9b--the-boards-live-channel) — it is a second live surface with its own architecture rather than a last item on the board's list, and the board is a finished single-author surface without it.

**Explicitly not in M9:** the outline, grouping, board naming or switching — multiple boards are modelled, one is shipped.

**Progress.** The server side is built — aggregate, catalog with a piece-to-boards index, find-or-start, REST, and three listeners including `unpin-discarded-piece`. The client renders the join: cards at their spots, a list of pieces not yet on the board, and pinning from it.

Dragging is in, and the interaction model landed where a canvas app puts it rather than where a first draft does. **The whole card is the handle** — press anywhere, move more than four pixels and it is a drag, less and it stays a click. A **single click selects**, a **double click opens**, arrow keys nudge the focused card one grid cell and shift leaps eight, and Escape or a press on bare board lets go. The three items owed with the drag pass are all paid: unpinning, a tolerance test that pins an id which was never captured, and a "Back to the board" link beside "Back to the pool" — both always offered rather than guessing where you came from, which keeps the URL clean and is never wrong.

**Snapping is a view decision, and it stays one.** Spots snap to a 5px grid, but `Spot` is still pixels and the client quantises on drop — the aggregate takes whatever integer it is handed. Obsidian Canvas does the other thing, where one coordinate *is* a 10px cell, and that was deliberately rejected: it makes the grid size part of what is stored, so changing it later, or making it zoom-dependent, silently reinterprets every spot in history — an event upcast to pay for a presentation choice. Position already lives on the board rather than on the piece because arrangement is a view concern; how the board helps you *aim* sits one level further out still.

The surface itself is now the Obsidian-style dotted canvas rather than a ruled grid, a long title **scrolls inside its card** instead of stretching it (the scrollbar is thin and only inked on hover), and a card being dragged **rides above the rest** so it is never lost behind one pinned later.

**Seven defects, none of which a test would have found first.** All of them came from opening the page and pushing cards around:

- A **cancelled drag left the card stranded.** `pointercancel` was unhandled, so `carrying` stayed set: the card sat at a spot that was never saved, and every later mouse move over it kept dragging with no button held. `pointercancel` now clears the carry, and `pointermove` returns before touching a signal `pinned()` reads — an unguarded write re-ran the whole pool join on every mouse move.
- A **pinned piece could land where nobody could reach it.** Four columns of 200px on a board about 620px wide with `overflow: hidden` clipped the fourth piece away with no pan to go and find it. Three columns now, and the board scrolls.
- **A drop flashed back to where it came from** for the frame between clearing the carry and the `PATCH` returning. Moves are applied locally first and rolled back if the request fails, which also makes an arrow-key nudge instant instead of a round trip.
- **The selection outline blinked off mid-drag**, because the board's own press handler cleared it before the card's click could set it again. Selecting happens on press now, and a card's press stops propagating.
- **The title carried its own focus ring** inside the card's, because the anchor is still focusable by mouse even at `tabindex="-1"`. The card is the focus surface, so the anchor's outline is suppressed.
- **Pieces could be pinned before the board had loaded**, and those pins were swallowed without a word — the waiting list was derived from the pool alone, so it offered every piece while `board` was still `None`. It waits for the board now, which is also the only point at which it can say anything true about what is unpinned.
- **Pinning several pieces quickly lost some of them, silently.** Two separate causes, both real. See below.

**Why quick pinning lost pieces**, because it is the one worth remembering. Six pins fired together produced `200 412 412 200 200 200`: concurrent commands on one aggregate collide on optimistic concurrency, and pins do not conflict *in the domain* — different pieces — but the aggregate is the unit of concurrency, so any two concurrent appends refuse each other. `EventSourcingService::execute` now **reloads and re-decides on a version conflict**, up to four tries; `execute_at` deliberately does not, because a caller who named a version through `If-Match` must hear that it moved on rather than have it papered over. Underneath that sat a second bug: the client took every response as truth, so an older board arriving late overwrote a newer one. Responses carry a version, and whole-state responses can be guarded on it — unlike the delta case argued in M8, where a version guard cannot rescue an out-of-order apply.

The board's scroll is a stopgap that pan/zoom replaces. Note for whoever writes that test: `overflow: hidden` still reports the full `scrollWidth` and still obeys `scrollLeft` and `scrollIntoView`, so a test built on any of those passes either way. Only a real wheel event tells the two apart — and Playwright's `mouse.wheel` drives the vertical axis but not the horizontal one, and needs polling because the compositor scrolls a frame later.

**The action bar is in**, floating over the selected card the way Obsidian Canvas does: rename, open, unpin. Unpinning moved off the card and onto it, so a card is a title and nothing else again. The bar flips below the card when there is not enough room above, and steps aside entirely while a card is being dragged.

**Renaming settled the question selection was posed to answer.** A rename opens a textarea over the card, commits on Enter or on clicking away, and abandons on Escape — and it is deliberately *not* a child of the card. That mattered more than it sounds: the first version put the editor inside the card and hit two versions of the same bug, both found by pushing cards around rather than by any test.

- Every press rebuilt the card's children, because `pinned()` read `carrying` and so re-ran the whole list on each pointer event. The fix pushes the drag position down into a per-card reactive attribute, so a drag now touches one `style` attribute rather than rebuilding the list — which also retires the per-mousemove join over the pool. Symptoms before that: double click stopped opening a piece, dragging by the title broke, and Escape stopped deselecting, all because the focused node was replaced underneath the gesture.
- Then the editor, moved out to board-level chrome, was still rebuilt whenever the board changed — so pinning another piece mid-rename reset what had been typed. What the editor needs is captured when it opens (`Renaming { piece, at, was }`), so it now depends on nothing but itself.

There is a lesson in both worth keeping: **a reactive slot that reads a broad signal will replace its DOM on every change to that signal**, and anything living in that DOM — focus, a caret, half-typed text, an in-flight gesture — goes with it. Read narrowly, or capture what you need up front.

The bar also shipped a bug straight onto another page: `.actions` was a bare class in a stylesheet that is concatenated globally, and `_boards.scss` is loaded after `_projects.scss`, so the workspace's project-row menu was absolutely positioned over the page and Save landed on top of Cancel. Board-surface rules are now nested under `.corkboard` and named for it. The second collision of this kind — [the convention is owed](./TODO.md).

**Cards the author can resize** was the first thing in this milestone to reach past the client. A placement is now a box: `PositionedPiece` carries a `Size`, board state is an `IndexMap<PieceLink, Placement>`, and eight handles on each card drag any side or corner, snapped to the same 5px grid with a floor of 80×40 so a card cannot be shrunk into nothing.

Two decisions worth keeping. **A size is stored in pixels**, for the same reason a spot is: store it in grid cells and the day the grid changes, every card in history silently means something else. And **`PieceResized` is a sibling of `PieceMoved` rather than a wider `PieceMoved`**, because dragging the left edge changes the origin *and* the extent — two facts, so two events. What made that awkward is that they are one *gesture*, and splitting it across two requests would be neither atomic nor honest. So `BoardCommand::Move` became `Reshape { piece, to: Option<Spot>, size: Option<Size> }`: one command per gesture, carrying only the parts that changed, emitting only the events that are true. Dragging a corner writes both and the board's version moves by two.

The optional parts are what keeps the write small — a body drag never sends a size, so it cannot clobber someone else's resize. That mattered enough to prefer it over the tidier "always send the whole box", which would have been last-write-wins across both.

**No upcast, deliberately.** `PIECE_PINNED` gained a field, which is exactly the shape `patches()` exists for — but nothing is durable yet, so there are no old events to patch and the ceremony would have been for its own sake. The debt of shipping one real patch stays open, and the first persistent store is when it comes due.

**The board pans and zooms**, so it is finally the infinite surface the milestone promised rather than a 620px box with a scrollbar. Drag the bare board to pan, ctrl and the wheel zoom at the pointer, and a small cluster in the corner reads the zoom and resets it. A plain wheel is deliberately left alone: the board sits inside a scrollable page, and a surface that swallows the page's scroll to move itself reads as the page being broken rather than as panning. Resetting the zoom undoes the zoom and nothing else — going back to 100% is not the same as going home, so the board stays where it was panned to — and every zoom control anchors on the middle of the view rather than the top-left corner.

The viewport's pan is a float even though a spot is an integer, which is not fussiness: rounding it meant `on_board` quantised the anchor before the pan was recomputed, so zooming in and back out crept a couple of pixels each time. Zoom now round-trips exactly, and a test asserts that by checking a board point lands on the same screen pixel before and after six zooms.

**Chrome does not scale.** Cards live inside the transform, the action bar lives outside it in screen coordinates. Zoom out and the cards shrink while the buttons stay the size a finger expects — Obsidian's behaviour, and the decision that fixes where the coordinate conversion sits. The rename editor deliberately goes the other way: it *replaces* a card visually, so it scales with one.

That conversion is now its own module, `boards/viewport.rs`, holding `Viewport { pan, zoom }` and the screen↔board arithmetic. It arrived with a discovery worth more than the module: **the client's unit tests run natively.** `cargo test --workspace` builds the client for the host and runs anything under `#[cfg(test)]` — the whole codebase had been excluding it and testing the client only through a browser. Pure logic like a viewport transform does not need one, and eight tests now cover round-tripping, zoom-at-the-pointer and the zoom limits without Playwright ever starting.

**Moving a card brings it to the front, and that is a fact the log records.** `PieceRaised` is a sibling of `PieceMoved`, emitted by `Reshape` when the spot changed and the card is not already topmost — the aggregate's `IndexMap` insertion order *is* the z-order, so raising is a re-insert. Resizing deliberately does not raise: stretching a card is not reaching for it. The alternative, folding the reorder into `apply(PieceMoved)`, was rejected for hiding a board rule inside a projection function where replay correctness would quietly depend on it.

**The client was split before the live channel arrived**, as planned: `board.rs` went from 983 lines to six modules, and the two pure ones — the viewport transform and where a drag or resize lands — carry 23 native unit tests that used to need a browser. See [the TODO](./TODO.md).

**Left in M9:** the live channel, and only that. Three of the four done-when criteria are met — one event per drop rather than per frame, a discarded piece leaving the board with no compensating event, and a project without a board getting one on first open. The fourth, *two browsers dragging pieces on one board and seeing each other live*, is the `Delivery::Fleeting` subscriber that has not been built.

### Milestone 9a — The shell

**Done.** Built in the direction the design pass settled on, because rebuilding the shell is exactly when the visual language gets set.

**Goal:** an app that uses the room it has, and a way between the views a project will accumulate.

**Build:**

- ~~**The project's home becomes the board**, not the pool.~~ **Done.** `/projects/{p}` is the board; the pool moved to `/projects/{p}/pieces`.
- ~~**A view switcher.**~~ **Done.** A masthead carrying the wordmark, the project's name and text tabs for the views that exist. M10 adds a tab and nothing else.
- ~~**Width becomes a property of the view.**~~ **Done.** `main` is now a flex column filling the window; the board grows into it, and the pool, the workspace and a passage each opt into `.column`.
- ~~**A theme the author can override.**~~ **Done.** Three states — light, dark, follow the system — as a segmented control in the masthead, remembered in `localStorage`. *Follow the system* is the absence of `data-theme` rather than a third palette, so the media query keeps working and there is exactly one place a theme is decided.

~~**Capture moves onto the board.**~~ **Done.** Double-click bare board and a card appears there, already in editing mode; type a title and it is captured and pinned in one gesture; cancel and nothing was ever recorded. That last clause is the design: the card is local until it is committed, so a cancelled capture leaves no piece behind and no event in the log. It needed no new machinery, as predicted — the rename editor was already a textarea floating over a card at a spot, so the two became one `Naming` with two shapes: `Capturing { at }` for a card that does not exist yet, `Renaming { piece, at, was }` for one that does. What differs is only what committing means.

**The gesture had to be told apart from every other double-click.** Cards, the action bar, the zoom control and the editor itself all live inside the corkboard, so a naive handler would have made a card whenever you double-clicked any of them. The test is `event.target() == event.current_target()`, which works precisely because `.surface` is a zero-size origin element and therefore has no hit area of its own — bare board really does mean the corkboard element itself.

**The ordering care named above turned out to be the real design.** Capture writes `pieces`, pinning writes `boards`: two aggregates, two commands, no transaction between them. If the pin fails the piece is already captured, and the decision is to *keep* it — it appears in the waiting tray, where the author can pin it by hand. The alternative, deleting the piece to make the pair atomic, would throw away the idea to tidy up the board, which is exactly backwards for a tool whose whole promise is that ideas do not get lost.

**Double-click on a card now edits its title**, replacing double-click-to-open, and **Enter on a focused card follows it** rather than diverging. That leaves the action bar's Open as the only way into a passage — which was fine for the mouse and broken for the keyboard, because the bar only appeared on pointer selection. So focus now selects: Tab to a card and its actions appear. Without that, making Enter rename would have removed the keyboard's only way to open a piece.

**Double-click means edit, everywhere.** On bare board it makes a new card; on a card it edits that card's title in place. Opening the passage moves entirely to the action bar's Open button. This replaces the current double-click-to-open, and it leaves one sub-question for the build: Enter on a focused card opens today, and it should probably follow the double-click rather than diverge from it — which would leave the bar as the only way in, reachable by Tab.

The one thing capture needs is care about *order*: capturing writes to `pieces` and pinning writes to `boards`, two aggregates and two commands, and a piece captured but not pinned is a piece stranded in the pool. The waiting list stays regardless — it is where pieces live that exist without being on the board.

**The look is settled: Ink & Ochre**, chosen from three directions mocked side by side. It takes its palette from the logo rather than from the greyscale the app drifted into — cream paper, deep teal ink, ochre accent — and it is deliberately the least disruptive of the three, since the current warm-stone palette is already halfway there.

| | light | dark |
|---|---|---|
| paper / raised | `#fbf7f0` / `#fffdf9` | `#14232a` / `#182b33` |
| ink | `#1d3b47` | `#e8e3d9` |
| muted | `#6f6659` | `#8fa3ac` |
| line | `#e6ddcf` | `#263b45` |
| accent | `#8c5f27` | `#d3a05c` |

**Type: Newsreader for headlines, Lexend for reading.** Georgia goes. **This is the app's first webfont**, and that has two consequences worth naming now rather than later. A local-first tool that fetches its faces from Google on every load is a contradiction, so both get self-hosted from the start. And the project is MIT while both faces are open-font-licensed — compatible, since the OFL governs the font files and the MIT the code. Both licences now ship in `clients/web/fonts/`, and the Reserved Font Name worry turned out not to bite: **Lexend reserves only "RevReading Lexend"**, and **Newsreader reserves nothing at all**, so neither name is encumbered. Nor is anything subset by hand — what is served is Google's own unmodified `latin` and `latin-ext` cuts, four `woff2` files, both faces variable so one file carries every weight. The `unicode-range` split means a reader who never types outside Latin-1 downloads 172 KB rather than 293 KB.

**The palette above is the corrected one.** As drawn, six of ten pairs failed WCAG AA and the fix was not cosmetic: the mockups carried *three* values for one role — `#8a8175`, `#b3aa9b` and `#c4bcae` were all "secondary text", none chosen, each invented where it was needed. That is the duplicate-by-invention habit the shell exists to end, so the answer was one muted value per theme rather than three nudged ones. Light muted went `#8a8175` → `#6f6659` (3.59:1 → 5.29:1 worst case across paper and raised), dark muted consolidated onto the `#8fa3ac` that already passed at 5.59:1, and the accent went `#9a6a2f` → `#8c5f27` (4.39:1 → 5.20:1) so that one value serves as text *and* as a border, instead of needing a second accent for each threshold.

**The switcher lists only views that exist.** Timeline, Threads and Cast are not drawn until they are built — a permanently dead tab is clutter in a tool used daily, and it was also two of the six contrast failures, since ghost text is unreadable by construction.

**Two things noted and deliberately not taken.** The chosen direction puts the view switcher in the top bar as text tabs; a left icon rail scales better once Timeline, Threads and Cast arrive, so revisit it at the fourth view rather than pre-building it. And monospace for anything countable — word counts, piece counts, the zoom reading — was the strongest single idea in the direction that lost, and it costs nothing here if it is ever wanted.

**The masthead had to become universal for the theme control to have a home.** It was built inside a project — wordmark, project name, tabs — and the workspace had no chrome at all, so a *global* preference had nowhere to live that did not vanish when you left a project. So the bar is now on every page: wordmark and theme always, project name and tabs only inside a project. Two things fell out of that, neither optional. The workspace said "Weaveling" twice, once in the wordmark and once as a 2.9rem hero forty pixels below it, so **the hero became "Your projects"** — the masthead names the app, the page names the page, the same split the project views already had. And the 404's escape hatch was a second link reading "All projects" beside a wordmark labelled the same, so it is now "Back to your projects".

**`<main>` was holding the site chrome.** The masthead is a `<header>`, which only maps to the `banner` landmark when it is *not* inside `<main>` — and `App` wrapped every route in one. The test asking for `getByRole("banner")` failed and was right to: the bar was not a landmark, and `<main>` claimed the chrome as page content. `App` now renders the router bare, each page emits `<header>` then `<main>`, and the flex column that fills the window moved up to `body`.

**Optical centring, not box centring.** The masthead's boxes were centred and its lettering still sat three pixels high, because a font reserves descender space that words like "Board" never use. The fix is `text-box: cap alphabetic`, which trims each line box to its cap band so flex centring lands where the eye reads it. Chrome and Safari honour it; Firefox has not shipped it yet and falls back to the old, slightly high text rather than to anything broken. The lesson generalises past this one bar: a suite that asserts on roles and text passes happily while the layout is wrong, so `shell.spec.ts` measures — the board's box against the window's, the column's margins against each other, the cap bands against the bar's middle.

**Done when:** ~~clicking a project lands on a full-width board; you can move between a project's views without going back through the pool; a passage still reads in a column; and the theme can be set against the system's wishes and survives a reload.~~ **All four hold.** 125 browser tests, 544 unit tests.

**Explicitly not in M9a:** translation, touch, and the outline itself.

### Milestone 9b — The board's live channel

**Probably next**, now that [M10a](#milestone-10a--one-tray-in-one-place) has settled where the tray lives.

**Goal:** two browsers on one board, seeing each other work.

**Build:** a second live surface. Awareness is bound to `/sync/{passage}` today and a board is not a passage, so the board needs its own socket carrying two different kinds of traffic: **committed events**, which are already published (`board.#`) and want a `Delivery::Fleeting` subscriber rather than a durable queue, and **in-flight drags**, which are awareness — a card travelling under someone else's pointer, never written down. The earlier implementation had precedent worth copying: a WebSocket pushing event messages, with per-event handlers patching a local store on the client.

**Rust you'll meet:** a second WebSocket surface that is deliberately *not* a CRDT, and awareness carrying something other than a cursor.

**Done when:** two browsers drag pieces on one board and see each other live; the durable record still holds one event per drop rather than per frame; and a peer that drops out leaves no ghost card mid-drag.

**Know before starting.** Two things are already recorded and both bite here. `InProcessDispatcher` delivers synchronously, so it will **hide** the very races this exists to expose — the board's projection never lags in a test, and the find-or-start race stays invisible. And the client's board state gains a second writer: every optimistic write, rollback and version guard in `open_board.rs` was written assuming this author is the only one moving cards. The `Reshape` command carrying only what changed was chosen with exactly this in mind, and it gets its first real test here.

**Explicitly not in M9a:** presence beyond the board, cursors in prose (that is the passage socket's job, already built), and any attempt to make drags durable.

### Milestone 10 — The outline

**Done.**

**Goal:** manuscript order, as a view over the pool rather than a property of it.

**Build:** `features/outline` — **sections** that nest and hold pieces, rather than nesting the pieces themselves. That was decided after the milestone was written, and it changed the shape: a section carries its own title, because a table of contents rarely wants the working title of the idea a scene grew from, and because the structure of a book is not the structure of the ideas it came from. See [The outline arranges sections, not pieces](./ARCHITECTURE.md#the-outline-arranges-sections-not-pieces) for the full reasoning and the guardrails.

Moves are shaped like an author's intent (*move*, *promote*, *demote*) rather than the "swap two nodes" primitive the earlier implementation was cornered into by its data structure. **Promote takes the sections that followed it along as children**, because the alternative silently reorders the book.

**The whole back end is done** — `features/outline` in the same five-crate shape as `boards`: core (aggregate, catalog port, service), contract, and adapters for catalog, messaging and REST, wired into the composition root. 92 tests across the feature. The aggregate covers both orderings, the promote-adoption rule, subtree moves, the cycle refusal, removal lifting children into place and a snapshot round trip; the integration tests drive all of it through HTTP, and one of them replays the log through a **service that never saw the writes**, so the structure and the reading order have to come back out of the events rather than out of whoever wrote them.

**One projection subtlety worth remembering:** the attachment index has to wake on `SECTION_REMOVED` as well as attach and detach, because removing a section returns its pieces to the pool. Listening only to the two obvious events leaves `outlines_holding` claiming pieces the book no longer contains — and the discard cascade reads that index.

**The client view is a real tree, chosen from three drawn alternatives.** The first attempt drew every row as a bordered input, which made the whole view read as a form rather than a book, and offered a "put it here" button beside *every* section whenever a piece was picked up. Both were rejected on sight. The chosen direction draws elbow connectors and a vertical rule per level, foldable twisties, pieces as leaves, and chrome only on hover — the structure is the subject.

**Keyboard-first, which is why `Promote` and `Demote` are commands.** Enter adds a sibling, Tab demotes, Shift+Tab promotes, Escape stops editing. Each row also carries the three actions as buttons, **disabled when they would do nothing** — a section at the top cannot be promoted, the first of its siblings cannot be demoted, and the button says so rather than silently doing nothing. **Focus survives a promotion**, which is the property that makes the model work at all: Tab, keep typing, and the letters land in the row you just moved.

**Sections drag, with three drop zones per row** — the middle nests the dragged section inside, the top and bottom edges insert it before or after as a sibling, and a section dragged into its own subtree is offered no landing at all rather than being refused after the fact. They also reorder with Alt+Up and Alt+Down, and with buttons that disable at the ends. That is the general `Move` command — which had been built all the way through the aggregate, service and REST surface and was reachable from nowhere, because the first client view never called it.

**Pieces reach the book by dragging from a rail**, or by clicking a piece and then clicking a section — one gesture, one state, so the keyboard and touch are not stranded by the drag. The section under the pointer lights up as the landing.

**Folding is local and nothing else.** Not in the aggregate — [that was the rejected design's structural failure](./ARCHITECTURE.md#the-tree-is-a-view-not-the-model) — and deliberately not in awareness either, because nobody wants their outline folding itself to match a collaborator's. A test reloads the page to prove a fold was never written down.

**The wordmark is the real logo**, served as a file and drawn with `mask-image` so it takes `var(--accent)` and follows the theme instead of carrying a hardcoded fill.

**Icons are inline stroke SVG on a 24px grid**, not an icon font and not text arrows. The first version used literal `←` `→` `×` characters in the body font, which is why they looked thin and mismatched. An icon font would be a second face to self-host and licence-check, it flashes before it loads, and a screen reader can read a glyph as a letter.

**An empty section says so** — a hollow mark and the word *empty* in the alarm colour, rather than the dashed red box the first attempt drew around every row. A book being planned is mostly holes; shouting about all of them is noise. Nothing refuses it, exactly as decided.

This is the privileged view: export needs a linear order, so the outline is what "the manuscript" means. A piece may sit on the board and be absent from the outline — it simply is not in the book yet.

**Still open:** the split-piece mechanics — see the design threads in [TODO.md](./TODO.md).

**Not in M10:** undo/redo. The event stream makes it available whenever it is wanted, which is exactly why it does not need to be built alongside the outline.

**Done when:** ~~a book-shaped outline of chapters and scenes, each openable in the editor, structural changes visible in the audit log, and rebuilding the projection from scratch reproducing the same order.~~ **All four.** 147 browser tests, 633 unit and integration tests. A leaf is a real anchor into its passage, so it is ctrl-clickable and its link copyable — the same rule the board's cards follow.

### Milestone 10a — One tray, in one place

**Done.** Small, and it removed an inconsistency an author met immediately.

**Goal:** the pieces waiting to be placed sit in the same place in every view.

The board grew its waiting strip along the **bottom**; the outline grew its rail on the **right**. Both hold the same thing — pieces in the pool that this view has not arranged yet — and an author moving between the two has to look somewhere different for it each time. The outline's rail is the better of the two: a vertical list reads a column of titles far better than a wrapping row of chips does, and it leaves the working surface its full height. So **the board's tray moves to the right** to match.

That also settles [the open question about the board's tray](./TODO.md) — it currently spends 80px of the working surface saying "Every piece is on the board", and a right rail can simply be narrow and quiet when empty rather than being a strip that has to justify its height.

**Make it responsive at the same time**, because the answer differs by width. On a narrow screen neither a bottom strip nor a side rail works: the tray wants to be a **drawer** — pulled in from the bottom or the right, over the surface rather than beside it, dismissed when you are done placing. That is the first piece of [M15](#milestone-15--touch-and-small-screens) worth building early, because it is the one the two views must agree on.

**Done when:** ~~the board and the outline present waiting pieces the same way; the board keeps its full height when nothing is waiting; and at a phone width the tray is a drawer in both.~~ **All three.** 152 browser tests, 633 unit and integration tests.

**One tray, defined once.** `.tray` lives in its own stylesheet and `laid_out` in its own module, so the grid, the toggle and the drawer are written down once and the two views cannot drift apart again — which is exactly how they drifted in the first place. Each view passes only what it keeps in the tray and how many pieces are waiting.

**The breakpoint is 60rem**, and the switch is a real change of kind rather than a narrower rail: above it the tray is a grid column beside the surface, below it the surface takes the full width and the tray becomes an overlay that slides in from the right. The toggle carries the count, so an author on a phone can see there is something waiting without opening anything.

### Milestone 10b — The outline's live channel

**After [M9b](#milestone-9b--the-boards-live-channel), deliberately.** The board's channel is the harder one to design — free placement, drags emitting thousands of frames a second — and whatever it settles about transport, awareness and a second live surface, the outline reuses rather than re-decides.

**Goal:** two browsers on one outline, seeing each other restructure.

**But the conflict story is the opposite of the board's, and that is the whole milestone.** [Moves of different pieces on a board commute](./ARCHITECTURE.md#the-event-catalogue), which is why `PieceMoved` takes no strict version check and concurrent drags are last-drop-wins — no work is lost, the card simply lands where the last author dropped it. **Tree moves do not commute.** Two authors moving sections at once can produce a cycle, or leave a section parented to one that has just been removed, and "last writer wins" on a tree can silently discard a whole subtree's placement. A position is safe to overwrite; a structure is not.

**This is where the intent-shaped commands earn their keep**, and they were chosen partly for it. A client-computed `Move { under, after }` is a placement derived from a view that may already be stale, so under concurrent editing `NoSuchNeighbour` starts firing in earnest. `Promote` and `Demote` carry only a section id and compute the placement from state at the moment they are applied, so they cannot go stale — which is the same reason [`Reshape` carries only what changed](#milestone-9--the-board-). Expect the [retry on version conflict](./ARCHITECTURE.md) to matter far more here than it does on the board.

**One piece of view state is neither durable nor shared:** which twisties are open. It is not in the aggregate — [that was the rejected design's structural failure](./ARCHITECTURE.md#the-tree-is-a-view-not-the-model) — and it does not belong in awareness either, because nobody wants their outline folding itself to match a collaborator's. Local only, and worth saying out loud because "not durable" and "therefore awareness" is the easy wrong step.

**Done when:** two browsers restructure one outline and see each other do it; no sequence of concurrent moves can produce a cycle or orphan a subtree; and the durable record still holds one event per move rather than per frame.

### Milestone 11 — The real store

**Done.** 788 tests with `--all-features`, 657 without; the API runs on PostgreSQL end to end.

**Goal:** prove the abstractions were worth the trouble.

**The database is PostgreSQL, and MongoDB is closed rather than parked.** Three things decided it, none of them preference. A `unique (aggregate, kind, version)` index *is* optimistic concurrency, so `append` needs no read-then-write and no lock — Mongo needs a multi-document transaction, or a stream collapsed into one document, which caps a stream at 16MB and fights the range reads and pruning the port declares. The outbox insert has to share the event's transaction, and Mongo's multi-document transactions and change streams both require a replica set even single-node, so the guarantee [M8 deferred the broker for](#milestone-8--messaging-) would arrive with ceremony attached. And the shape of the data settles the rest: event payloads are JSON, which `jsonb` stores and indexes natively, but the *access pattern* is a strictly ordered append-only log read by version range, which is relational's home ground rather than a document store's. Mongo would win on sharding and on read models that drift; the catalogs here are `(id, project)` pairs.

**Stored events get a shape of their own, per feature.** No `core` crate depends on serde — `PieceEvent`, `BoardEvent` and `OutlineEvent` are pure domain enums, 31 variants between them — and in-memory never had to care because it only clones. Deriving serde on the domain enums is the cheap answer and the wrong one: the enum's field names silently become the on-disk format, so a rename in `core` breaks every stored event with no version bump to trigger the patcher, which is exactly the failure [the abandoned implementation's never-once-run patcher](#milestone-7--event-sourcing-for-real-the-pool-of-pieces) predicts. So the SQL is written once against a raw row, and each feature supplies a codec to and from a stored shape it owns — the same shape `OutlineEventDTO` and `body()` already have for the broker, but total and reversible where publishing is lossy on purpose.

**Build:** a second backend for every port that has one — `ProjectStore`, `PassageStore`, the event store — as modules behind an optional cargo feature, not sibling crates. CI must run `--all-features` or none of it is type-checked.

**This is where storage representation finally gets decided,** and where the transaction tests that in-memory cannot express have to be written: rollback, connection failure mapping to `StoreError::Backend`, and the concurrency guard `apply` needs if `PassageStore` goes the snapshot route. The event store's `append` must be atomic across the version check, the append **and** the outbox insert — one transaction, invisible above the port.

**The steps, each reviewable alone:** ~~the rig~~; ~~the event store against its existing conformance suite~~; ~~the outbox and its relay~~; ~~`PassageStore` and the snapshot-versus-log choice~~; ~~the catalogs and `ProjectStore`~~; ~~the three stored-event codecs~~; ~~the wiring~~; ~~the background tasks and CI~~.

**Owed to step 6:** the API should ensure its own databases exist at startup, beside the migrations it already has to run. `compose.yaml` creates them today by reading `features/`, which keeps the list from going stale but cannot help with a feature added after the volume exists — PostgreSQL runs an init script only on an empty data directory. The authority belongs where the list already lives, in `Adapters`.

**The rig is done.** `compose.yaml` brings up PostgreSQL 18 and creates a database per feature, and `libraries/test-harness` hands each feature a namespace of its own — created and dropped around the test — so the suites still run in parallel and no test can see another's rows. A namespace left behind by a killed test is swept an hour later by whichever test runs next. Database tests sit behind a `postgres` feature, so `cargo test --workspace` needs no Docker and `--all-features` runs everything.

**A database per feature, decided partway through and applied backwards.** The first cut had one database with one `events` table for everything, and that is defensible on its own terms — every query is prefixed by the aggregate id, so a million events answer a stream read in 0.6ms and table size only buys btree depth. What it does not do is stop a foreign key from crossing a feature boundary, which is the one way to re-couple two features that no Rust rule catches, and step 5's catalog tables are the first moment anyone could write one. So the boundary moved into the database, where PostgreSQL enforces it — see [the dependency rules](./ARCHITECTURE.md#dependency-rules). The cost is a pool and a migration set per feature; the gain is that extracting a feature into its own service becomes `pg_dump` rather than a filtered copy, and stays that way without anyone remembering a rule.

**The event store is done, and the conformance suite really is the same suite.** `conformance_tests!` now takes a workbench rather than a store expression, so one list of cases runs against both backends and a case added later cannot quietly skip PostgreSQL. `unique (aggregate, kind, version)` carries the concurrency, so nothing locks: two writers at one version race in the index and exactly one wins, with the loser getting `Outdated` rather than a backend failure. `append` reads the stream's head, compares it to `expected` in Rust, and writes its events — all inside one transaction.

**The guard was checked by breaking it.** The first version did the check in SQL, as a two-branch `case` inside a single `insert ... select ... where` fed by seven parallel arrays through `unnest` — one round trip, and unreadable: ten positional parameters whose meaning lived a hundred lines away in the `bind` chain, with nothing checking that the two agreed. Deleting a branch showed why it mattered, and also that it was doing too much work: `coalesce(max(version), 0)` compared against `expected` in Rust answers all of it in one line — an empty stream, a pruned one, and a caller that is simply wrong — which is what the in-memory store always did. Removing that one comparison now fails two tests: an `expected` *ahead* of the stream, and a stream that looks empty because pruning collapsed it to a snapshot, where appending at version zero would have written a second version 1 underneath the snapshot. Both are in the shared suite, so in-memory answers for them too.

**The cost of that legibility is N+2 round trips instead of one**, on batches that are almost always a single event, plus a transaction the outbox was about to require anyway. What it buys is two statements that can be pasted into `psql` unchanged, nine binds sitting against a nine-placeholder `values` list, and the end of a hazard nothing would have caught: seven `Vec`s that had to stay the same length and in the same order as the columns.

**One file per operation, and the SQL sits in it.** `postgres/` is `writing.rs`, `reading.rs`, `snapshots.rs` and `rows.rs`, with `mod.rs` down to a hundred-odd lines holding the store, its errors and an `EventStore` impl that forwards. A trait impl cannot be split across modules in Rust, so each operation pays for a forwarder repeating its signature — worth it, because reviewing an operation no longer means scrolling a hundred lines up to find which `$n` meant what. Consolidating the row readers took `read_through` off its own hand-written query and onto the same path as everything else.

**The event store's schema belongs to the event store**, so it lives in `libraries/eventsourcing/migrations/` and lays itself down in whichever database it is pointed at, tracked in its own `_sqlx_migrations_events` ledger. That leaves the default ledger free for the feature that owns the database, so a feature's catalog migrations sit beside the library's without either knowing about the other — there is a test for exactly that, because it is the load-bearing part of the arrangement.

**The dual write is closed.** `append` now writes the outbox row beside each event inside the same transaction, and a relay publishes from there — so the window in which an event was durable but unannounced is gone. Proved by breaking it: moving the outbox insert onto the pool instead of the transaction fails the test that a rolled-back append leaves no message waiting. Which event becomes a message is the feature codec's decision, not the store's — the mapping returns `Option<Message>`, so a snapshot and an unpublishable correction simply produce nothing.

**The outbox stores the message's identity, not just its content.** `message_id`, `conversation` and `caused_by` are columns, because a relay that minted a fresh id on every retry would defeat the [inbox that has to recognise a redelivery](./ARCHITECTURE.md#messaging--the-seam-now-the-transport-later). `claimed_until` is what stops a crashed relay stranding a message and a second relay double-sending one; `FOR UPDATE SKIP LOCKED` is what lets two of them run at all. One relay preserves order; concurrent relays do not guarantee ordering across aggregates, which is why handlers must stay idempotent.

**Publishing is chosen with the backend rather than by the service.** The Postgres store enqueues and the relay sends; the in-memory store keeps publishing through the service, because it has no durable state to protect — a crash there loses the events as well as the queue, so an in-memory outbox would guard nothing. Each feature's `Ports` constructor picks the pair together in [step 6](#milestone-11--the-real-store), so a double-publish cannot be wired by accident. This deliberately amends the sentence in [Transactions and Atomicity](./ARCHITECTURE.md#transactions-and-atomicity) about an in-memory adapter pushing onto a queue.

**Published entries are kept for 90 days.** PostgreSQL has no TTL index — that is a MongoDB feature — and the alternatives are `pg_cron` (an extension outside the base image, needing `shared_preload_libraries` and a restart, and holding application policy where the app cannot test it) or the application doing it. Since the relay is already scheduled work, `delete_published(before, at_most)` is one more statement it can run on a slower cadence, batched so the deletes never arrive as one lump. A partial index on `published_at` keeps the sweep off a sequential scan, and the retention window is `KEPT_FOR`, exported for step 6 to schedule. **An entry that was never published is kept however old it is** — that is a stuck message, not rubbish, and deleting it would lose it silently; there is a test for exactly that. Left unbounded, the outbox would have roughly doubled the storage of the event log forever: a million published entries measure 297MB against the events' own 265MB.

**A passage is an append-only log of updates, collapsed when the tail gets long.** The port was written so it would not have to choose — `apply` takes a delta and a full snapshot is just a delta from nothing — and what settled it was who calls `apply`: the sync socket, many times a second per passage and concurrently from several clients. One row per passage would make every keystroke read the whole document, CRDT-decode it, merge, re-encode and write it back under `SELECT FOR UPDATE`, serialising every writer on that passage and scaling with document size rather than edit size. Appending makes each write a single lock-free insert; `load` reads the newest snapshot plus everything after it, and every row applies to an empty document because a snapshot *is* an update.

**Compaction is opportunistic and costs no extra round trip.** The insert returns the tail length as a subquery in its own `RETURNING`, so `apply` stays one statement and only every Nth call takes the `passages` row lock to collapse. Removing that trigger fails the test that six writes do not leave six rows. **The conformance suite runs twice against PostgreSQL** — once appending only, once compacting after every single update — so all twelve cases have to hold whether or not the log has just been rewritten underneath them. Neither run touches the in-memory adapter, which still answers the same twelve.

**Two tables, and the foreign key is doing real work.** `passages` holds existence so `create` gets `Conflict` from a primary key and `delete` gets `NotFound` from a row count; `passage_updates` references it, so an update for a passage that does not exist is `NotFound` from a foreign-key violation rather than a separate existence query, and `ON DELETE CASCADE` means deleting a passage cannot leave parts behind that `load` would trip over. That key is legal because both tables belong to the same feature — [the ban is on crossing feature boundaries](./ARCHITECTURE.md#dependency-rules), not on relational integrity within one.

**The catalogs and `ProjectStore` went in without a surprise**, which is what the conformance suites were for: four ports, four existing suites, every case now running against both backends and each crate's test count exactly doubling. `remember` is an `ON CONFLICT DO UPDATE`, because a projection replays and has to be idempotent; `holds` replaces a board's or outline's pieces inside a transaction, because "replaced, not added to" is a suite case.

**No foreign key crosses a feature line, and one could not go inside a feature either.** The board and outline catalogs each hold two tables -- the summary and the piece index -- and the obvious key from the index to the summary would have broken a documented semantic: `holds` is called for boards nobody has `remember`ed yet, because the two are independent projections fed by different events arriving in whatever order the broker chooses. So relational integrity was right for a passage and its parts and wrong here, for the same reason in both cases -- whether one row is *part of* another or merely *about* it.

**The id columns are pinned to `COLLATE "C"`.** Every listing orders by an id, and base62 ids only sort by age under byte ordering -- a glibc or ICU locale puts `a` before `A`, and a newest-first listing quietly stops being newest-first. It cannot be caught here: the alpine image's musl locale already behaves like `C`, so the fault would appear only in production. A test asserts the collation on every id column rather than trusting whichever database it happens to run against.

**Every feature's events now have a stored shape of their own**, which is the decision taken at the top of this milestone finally cashed: `features/{pieces,boards,outline}/adapters/store`, each a mirror enum with serde derives and conversions both ways, 22 variants between them. `core` still has no serde anywhere. The value types travel as their own stored shapes too -- `Spot`, `Size`, `PositionedPiece`, `PlacedSection` -- so a field rename in the domain is a compile error in the codec rather than a silent change of format.

**A codec can refuse a row**, and that turns out to be the interesting part. `event` returns `Option`, so a stored title the domain would no longer accept, or a section id that no longer parses, reads as nothing rather than replaying an aggregate into a state its own rules forbid. Two tests I wrote asserted the wrong invariant and failed honestly: an untitled *piece* and an untitled *section* are both legal, because capture and "Add a section" both start blank and are named afterwards. The invariant that does hold is control characters, and both codecs are tested on it.

**`Codec` moved out from behind the `postgres` feature, because leaving it there quietly broke the build promise.** The codec crates need it and are not themselves optional, so they turned `eventsourcing/postgres` on for the entire workspace through feature unification -- `cargo test --workspace` went from 641 tests to 703 and started requiring Docker without anyone asking it to. `Codec` is a feature's declaration of its stored shape, not a PostgreSQL detail, and [M13's export](#milestone-13--local-mode) will want the same thing, so it belongs in the crate root with `serde_json` as an ordinary dependency. `MessageMapping` stays behind the flag, since it genuinely needs `messaging`.

**The API runs on PostgreSQL, and "one line in one manifest" turned out to be true.** Every feature has a `Ports::postgres` beside its `Ports::in_memory`, `Adapters::postgres` picks them all up, and `main` chooses by cargo feature. `Databases` is five pools -- one per feature -- and `Databases::ready` creates any that are missing and lays down every schema at startup, which closes the hole the compose init script left: a feature added after the volume exists now gets its database from the API rather than from `docker compose down -v`.

**The same five-pool struct is what makes the end-to-end test possible.** Production hands it five databases; the test hands it five *schemas* from one throwaway fixture. Nothing above the pool knows which it got, so the test drives the real HTTP API through the real adapters: a project written and listed back, a project outliving the server that wrote it, and every feature's rows landing in its own database and nowhere else -- capturing a piece leaves the boards database empty.

**And it exposed the gap 6c has to close.** With publishing chosen by the backend, the PostgreSQL store enqueues and the service publishes nothing -- so with no relay running, **no projection is ever fed**. Reading an aggregate still works, because that replays its own stream, but every listing that reads a catalog comes back empty. The test says so out loud rather than papering over it: a captured piece is readable by id and its message is sitting unpublished in the outbox. The application is not usable end to end on PostgreSQL until something drives `deliver`.

**The outbox has no foreign key back to `events`**, which the first draft of the migration gave it. Compaction deletes the events a snapshot replaced, and `on delete cascade` would have quietly unsaid messages that had not been published yet. An outbox row is a statement that something happened; pruning the record of it must not retract it.

**The relays run, and the chain closes.** `RelayTask::started` spawns two loops per outbox -- one delivering, one sweeping published entries past `KEPT_FOR` -- and `stop` ends both and waits for them, so `ctrl-c` releases the database instead of hanging. `main` starts them after `app()` has registered its listeners, which is the one ordering that matters: a relay publishing into a dispatcher nobody is listening to would drop the message on the floor.

**Which features have an outbox is the feature's own answer**, not the service's. Each wiring crate exposes `outbox(...) -> Option<PostgresOutbox>` -- `Some` for the three event-sourced features, `None` for `projects` and `passages` -- and the service starts whatever it is handed. Same shape as `lay_out` and `NAME`, and for the same reason: the composition root knows which features exist and nothing about what they are made of.

**The test that matters is the whole chain.** Capturing a piece through the real HTTP API now arrives in the piece catalog: append, outbox row in the same transaction, relay, dispatcher, projector, projection. That is the listing which came back empty at the end of the wiring step, and it is what made PostgreSQL unusable end to end until now.

**The relay is woken, not polled.** `append` runs `pg_notify` on the same transaction as the outbox insert, and the delivering loop waits on a `LISTEN` instead of on a timer -- so PostgreSQL holds the notification until the append commits, and a rolled-back append wakes nobody. That makes the poll a backstop rather than the latency, which is why `deliver_every` moved from 250ms to five seconds: what it now covers is a dropped listener connection, an expired claim and a publish that was refused, none of which is worth a quarter-second heartbeat. Proved by breaking it -- delete the `pg_notify` and the test that delivers against a five-minute poll interval fails. If the listener cannot be connected at all the loop logs it once and polls alone, because a relay that delivers late is better than one that does not start.

**The notification channel is per-schema**, `outbox_waiting_<current_schema>`, computed by the same expression on both sides so they cannot disagree. A channel is database-wide in PostgreSQL, not schema-scoped: with a database per feature that would never matter, but the test fixture puts every schema in one database, so a global channel had every relay waking on every other feature's append -- harmless in production, and fatal to a test that has to prove *this* append did the waking.

**CI exists for the first time.** `.github/workflows/check.yml` runs `fmt`, `clippy --workspace --all-targets --all-features`, the client's own wasm `clippy`, and `cargo test --workspace --all-features` against a PostgreSQL service container -- so code behind the `postgres` feature is type-checked and exercised rather than merely present. A second job installs Trunk and Chromium and runs the browser suite.

**Done when:** the suites pass unchanged against the real store, and swapping backends is one line in one manifest.

### Milestone 11a — Projects event-sourced, and the deletion cascade

**Goal:** the last CRUD feature joins the rest, and deleting a project actually leaves nothing behind.

Deferred out of [M8](#milestone-8--messaging-), where the cascade was originally scoped. Sits here because both halves start paying at the same moment: with a durable store — and certainly in [local mode](#milestone-13--local-mode) — an orphan outlives the session, and **deletion is exactly where an author wants an audit log**.

**Build:** ~~`Project` as an aggregate — `Started`, `Renamed`, `Deleted` — with the catalog, projector and `If-Match` handling that `pieces` already has, so the codebase stops carrying two shapes of feature. Then the cascade itself: deleting a project disposes its pieces, boards and passages, tolerating a project that never had a board.~~

#### Step 1 — `projects` becomes an aggregate — done

**`ProjectStore` is gone**, and with it the last CRUD port in the codebase. `projects` now carries the same five parts every other feature does: an aggregate, a stored-event codec, a catalog with two backends behind one conformance suite, a projector fed by messages, and a REST adapter that reports and demands versions. `Ports` no longer has a `store` — it has `events` and `catalog`, like the rest — and the feature returns `Some` outbox, so its relay starts with the others.

**The aggregate carries its own timestamps**, which `pieces` does not need to. An author sees "edited 2 hours ago" on a project row, so `created_at` and `updated_at` are state rather than something a reader derives — and they come from `EventMetadata.occurred_at` in `apply`, never from a clock the aggregate reaches for. The one place that cannot work is the snapshot: it is written at compaction time, long after the fact, so its own `occurred_at` is meaningless and both moments travel inside the snapshot body. There is a test that replaying a snapshot stamped years later still yields the original moments, because getting this wrong would silently re-date every old project the first time the log was collapsed.

**Three things about the API changed**, all deliberate and all visible to the client:

- **The listing is newest-first**, where the CRUD store returned creation order. It is now a projection ordered by id, and a v7 id sorts by age — the same rule the piece catalog follows, for the same reason: what an author touched most recently should be what they see first.
- **`ProjectDTO` gained `version`**, and the routes now speak `ETag`/`If-Match`. Renaming from a stale version answers `412` rather than silently winning.
- **A deleted project answers `404` on the way in and `409` on the way back.** `GET` is refused because the project is gone from the author's view; `PATCH` is refused as a conflict because the stream is still there and still says why. The old store deleted the row and had nothing left to say.

**A project's name collided with the event-name convention.** All four features tag their published event DTOs with `#[serde(tag = "name")]`, which is the event's own name — and a project's payload field is also `name`. serde refuses that outright rather than letting one shadow the other. The field is published as `project_name`; renaming the tag across four features and the library that derives it would have been the larger change for the smaller reason.

**And the postgres service test had to become honest**, the same way [the pieces one did in 6b](#milestone-11--the-real-store): with the store enqueueing rather than publishing, a project written with no relay running is readable by id and absent from the listing. The end-to-end relay test now follows a project through the whole chain as well as a piece, each on its own database.

#### Step 2 — the cascade — done

**Choreography was enough, and [M11b](#milestone-11b--one-flow-in-every-mode) is why.** The argument for a saga was that a cascade failing midway must be "recoverable and observable" — but a durable delivery queue with five bounded retries and a dead letter carrying its root cause already buys exactly that, per step, without a coordinator. So four listeners, each disposing its own and none knowing about the others: `discard-pieces-of-deleted-project`, `discard-boards-of-deleted-project`, `discard-outlines-of-deleted-project`, and `delete-passage-of-discarded-piece`. **What choreography still cannot say is when the whole cascade is *done*** — that, not recoverability, is the thing that would buy a saga its place, and nothing asks for it yet.

**Boards and outline had no way to end.** Neither aggregate had a `Discarded` event at all, so both gained command, event, guard, snapshot field, codec and contract — and both catalogs gained a `forget`, which runs the summary delete and the piece-index delete in **one transaction**, so a crash cannot leave the index naming something that is gone.

**The prose needed the event to carry it.** A passage is reachable only through its piece's `PassageLink`, and passages cannot look a piece up across the seam. The alternative was an index in passages fed by `piece.passage.attached`, whose failure mode is a *silent* leak — a missing entry orphans the prose with nothing to report. So `PieceEvent::Discarded` carries the passage it went with: self-sufficient, nothing to lag, no second projection. An event carrying a fact a different context cannot otherwise obtain is the case the ["what changed, not what it replaced"](../ARCHITECTURE.md#event-sourcing--discipline) rule was never about.

**One sweep is bounded, because one project can hold thousands of pieces.** [ARCHITECTURE says so in as many words](../ARCHITECTURE.md#event-sourcing--discipline), and discarding ten thousand in one handler would block every other listener behind it for the duration, outlive the 30-second claim, and hand the same delivery to a second consumer. So `pieces` takes 100 at a time and publishes `piece.sweep.more` carrying a **cursor** — answering the original message, so the whole sweep stays one conversation. The cursor is load-bearing rather than tidy: relying on the catalog shrinking would loop for ever, because the projector forgets discarded pieces asynchronously and a continuation can arrive before it has caught up. A query-plan test pins the cursored read to the index, since the failure there is silent — correct rows, cost growing with the table.

**And it found one that was nobody's cascade bug.** Both catalog projectors subscribed only to `started`, so a discarded board or outline could never leave its listing — latent since whenever discard-by-any-route arrived, and the cascade merely got there first.

**Done when:** ~~deleting a project leaves nothing behind; a cascade that fails midway is recoverable and observable; the project's own history says who deleted it and when; and `projects` looks like every other feature~~ — all four, proved end to end on PostgreSQL through the real API, with an assertion that nothing dead-lettered.

---

### Milestone 11b — One flow in every mode

**Goal:** in-memory enqueues and relays like PostgreSQL does, so local mode and server mode take the same path through the application.

**The problem is not durability, it is divergence.** Today the two backends differ in *where publishing lives*: the PostgreSQL store writes an outbox row and a relay publishes from it later, while the in-memory store publishes inline through [`PublishingEventStore`](#milestone-11--the-real-store). So a command in local mode has its projections up to date by the time it returns, and the same command on PostgreSQL does not. Every test that runs in memory — which is nearly all of them — exercises a flow no deployment uses.

**That is not theoretical; it bit twice in one milestone.** The wiring step left PostgreSQL with no relay, so no projection was ever fed and every listing came back empty; [M11a step 1](#step-1--projects-becomes-an-aggregate--done) hit it again the moment `projects` stopped publishing inline. Both were caught only because a handful of tests run against a real database in Docker. A bug that depends on a projection being one beat behind — a read straight after a write, a listener whose ordering assumption is wrong, a cascade step that races the projection it reads — is invisible to the in-memory suite by construction.

#### Step 1 — one flow — done

**The trait came first.** `Outbox` and `Nudges` now sit at the crate root, and `RelayTask` moved out of `postgres/` to drive either backend — so there is one relay, not one per store, and the PostgreSQL one keeps `LISTEN/NOTIFY` only as its own implementation of a wake-up. `tokio`, `tracing` and `messaging` stopped being optional dependencies of `eventsourcing`, which is honest: relaying is what the library does, not a PostgreSQL detail. Nothing reaches wasm from here, so the cost is nil.

**`Ports` carries its own outbox**, and the free `outbox(pool, publisher, clock)` per feature is gone. `Adapters::outboxes()` hands the composition root a `Vec<Arc<dyn Outbox>>` and `Relays::started` takes exactly that — so the service no longer knows which backend it is running, and local mode starts relays like everything else. `main` lost its `#[cfg]` split entirely.

**`PublishingEventStore` has no deployment left.** Both backends enqueue; the decorator survives only for a test that wants publishing without a relay.

**What it found, which is the point.** Five tests failed the moment delivery stopped being synchronous, and they were not all the same thing:

- **Three were tests observing a projection early** — fixed by a `settle()` on the test wiring, which drains the outbox rather than assuming it has drained. That is the discipline the PostgreSQL tests already followed.
- **One was a real idempotency defect.** `unpin-discarded-piece` read a stale index on a redelivered `piece.discarded`, tried to unpin a piece that was already gone, and was refused with `NotPinned`. It had only ever looked idempotent because the index was always current. Fixed.
- **Two were the find-or-start race**, and they are `#[ignore]`d rather than settled, because [TODO.md](./TODO.md) says in as many words that a settle hook would hide the defect the canary exists to find. `open` asks a projection whether the project already has a board; a second open that beats the projection starts a second one. It needs its own step and a decision.

The canary was documented as "one test leans on delivery being synchronous, and that is the whole list." It was five, and two of them were defects rather than test artefacts.

#### Step 1a — the uniqueness guard — done

**`open` claims instead of looking.** `BoardService::open` and `OutlineService::open` asked the catalog whether the project already had one and started one when it said no — so a second open that beat the projection started a second board. They now claim `(kind, project)` in a registry: an insert that returns the id already holding the claim when it loses, so the loser goes and reads the winner's board rather than being refused. Starting is then idempotent against the event store's own `(aggregate, kind, version)` guard — two racers both try `begin`, one wins, the other reads.

**It is a library, not a feature detail.** Boards and outline had the identical defect with the identical shape, so [`libraries/registry`](./libraries/registry) owns the `Registry` port, an in-memory and a PostgreSQL adapter behind one conformance suite, and the `claims` table's migration. Each feature lays it down in its **own** database, so the guard never becomes a shared table between features — and the ledger is `_sqlx_migrations_claims`, because a feature now runs three migrators into one schema and sqlx would otherwise see one version 1 modified into another.

**Proved rather than assumed:** a PostgreSQL test fires eight concurrent claims at one key and asserts all eight are told the same board. The three canary tests are un-ignored.

**And the catalog stopped deciding anything.** `in_project` remains as the listing the model anticipates, but neither service holds a catalog any more — identity comes from the claim, which cannot lag, rather than from a projection, which can.

#### Step 2 — `Kept` gets teeth — done

**The first attempt was in the wrong layer, and the reason is worth keeping.** Making a `Kept` refusal fail the publish closes the gap in process — and it is a shape that **cannot exist over a broker**. `publish` hands the message to an exchange; the broker takes it and returns; consumers have not run yet and may be on another machine, so there is no value a publish could return that describes how handling went. Worse, retrying the *message* redelivers it to listeners that already succeeded, which a broker never does. It reproduced the exact divergence this milestone exists to remove, one layer up. Reverted, then built where the broker puts it.

**The dispatcher became an exchange, and delivery became a queue.** `publish` writes one **delivery row per interested listener** and returns — that is the binding, and it reports the transport and nothing else. A `DeliveryConsumer` claims what is due, hands each delivery to its listener and settles it: handled deletes the row; refused counts the attempt and pushes `due_at` out by a growing backoff; the fifth refusal moves it to `dead_letters` with the reason. `Delivery::Fleeting` is dropped rather than retried, which is the first time that half of the enum has meant anything.

**So a retry reaches only the listener that refused** — each has its own queue and settles on its own, exactly as a consumer acks its own queue over RabbitMQ. There is a test that says so, because it is the property the first attempt got wrong.

**`Deliveries` is a port with two backends and one conformance suite**, in memory and on PostgreSQL, so local mode and server mode take the same path here too. The state is durable on purpose: an attempt count that resets on restart is not a budget, and a pending retry lost to a crash is precisely the message the mechanism exists to protect. That is also what settled [SQLite for local mode](#sqlite-backs-local-mode--decided).

**Messaging got its own database.** Deliveries cannot live in a feature's database — the dispatcher is shared, and a `piece.discarded` delivery belongs to a *boards* listener. So the broker has its own storage, separate from the application's, which is what a broker has anyway.

**It retires when RabbitMQ arrives, and only it.** Both its tables are things a broker owns — `deliveries` becomes queues with their own redelivery counts, `dead_letters` becomes a dead-letter exchange — and a RabbitMQ adapter would not implement `Deliveries` at all; it would replace the publisher and the consumer loop wholesale. Three things do not follow from that, and are worth writing down before someone assumes they do. The **outbox is untouched**: it lives in each feature's own database and guarantees the message *leaves*, which a broker makes more necessary rather than less. An **inbox appears but not here**: redelivery becomes guaranteed rather than ours to control, so ["remember the message id, reject what has already been seen"](./ARCHITECTURE.md#messaging--the-seam-now-the-transport-later) becomes real — and it has to be transactional with the handler's own write, so it belongs in the feature's database. And the **code stays alive for [local mode](#sqlite-backs-local-mode--decided)**, which has no broker: `Deliveries` keeps its in-memory and SQLite adapters, and it is the PostgreSQL one that goes. The port outlives the adapter, as everywhere else here.

**Two things fell out of it.** A **listener name is now a queue name**, so two listeners sharing one would quietly eat each other's messages — `listen` asserts against it, because a wiring fault must fail at startup rather than be debugged later. And a **dead letter records the root cause** rather than `NotHandled`'s own wording: a test asked why the message failed, got "catalogue-piece could not take in piece.captured", and that is not an answer, so the reason now walks the whole source chain.

**The consumer is woken like the relay is.** `enqueue` notifies — `tokio::sync::Notify` in memory, `pg_notify` on PostgreSQL — and `DeliveryConsumer::run` selects on that, a 200ms backstop look, and the stop signal. Without it the backoff curve would have been decorative: the drain ticked on the outbox's five-second backstop, so a delivery re-due in 200ms would have waited five seconds anyway. The `PgListener` wrapper moved into `messaging` and both queues share it, and `Notifications` moved with it — `eventsourcing` already depends on `messaging`, so one trait serves both rather than two identical ones.

**The backoff is 200ms growing five-fold** — 0.2s, 1s, 5s, 25s, 125s. Fast where the failure actually is (a projection a beat behind clears on the first retry) and slow enough at the tail that a database blinking does not dead-letter everything in flight. Both ends are asserted, because either alone is easy to wreck.

**The sweep of all eight listeners found one more, and the split is the useful part.** Six are **projectors** — they read `events.latest(...)`, the aggregate itself, and write a projection. A stale catalog cannot mislead them, which is the property already tested as "the projector reads the piece rather than trusting the message, so order cannot bite". The other two are **tidiers**, which read a projection to decide what to *change*: `unpin-discarded-piece` and `detach-discarded-piece`. Both had the same defect, and the second was found only by looking, because its test settled the projection *between* the two redeliveries and so never exercised a stale index at all. Moving one line made it fail, and it is fixed the same way — an already-detached piece is done, not a refusal.

**The rule that falls out:** a listener that reads the aggregate is safe by construction; a listener that reads a projection to decide on a command must treat "already done" as success. There are only two of the second kind today, and the cascade is about to add more. They are retried rather than lost now, which turns each into a delay rather than a defect — but a listener that will never cope still burns five attempts before dead-lettering.

**Done when:** ~~a listener that refuses is retried a bounded number of times and only then dead-lettered; a retry reaches **only** the listener that refused; `publish` still reports nothing about handling~~; and local mode surfaces the dead letter to the author instead of filing it for an ops team that does not exist.

---

### SQLite backs local mode — decided

**Decided 2026-09-29**, reversing the plan that [M13](#milestone-13--local-mode) was written against. Local mode was going to be *no database*: in-memory stores plus export and import of a project file. It will be **SQLite instead**, one file per feature module and one for messaging — the same database-per-feature shape the server has, with SQLite standing in for PostgreSQL.

**What decided it was the retry queue, not the event store.** The durability argument for the stores alone was weak: export and import narrows the window, and an author who loses an afternoon has lost an afternoon either way. But [M11b step 2](#milestone-11b--one-flow-in-every-mode) made delivery durable — a listener that refuses is retried up to five times and then dead-lettered, and **that state has to survive a crash or the guarantee is a lie in local mode.** Losing pending retries means losing exactly the messages the mechanism exists to protect: the ones that had not been handled yet. In-memory deliveries would mean local mode has the *shape* of the retry but not the promise, which is the divergence this whole milestone exists to remove.

**One file per feature, plus one for messaging**, matching the server exactly: cross-feature data stays unwritable rather than merely forbidden, and the broker's storage stays separate from the application's — so the [messaging database](#milestone-11b--one-flow-in-every-mode) is a file that goes away when a real broker arrives.

**The relay needs no `LISTEN/NOTIFY`.** SQLite has none — no server, so nothing to notify across connections, and `update_hook`/`commit_hook` fire only on the writing connection. *Revised 2026-10-07, M13 step 2b:* the plan was the `tokio::sync::Notify` the in-memory outbox uses, but that outbox's `enqueue` is a method holding the `Notify`, while SQLite's is a free function inside the writer's transaction with nothing to notify. The commit hook fires before the commit is visible, so a relay woken by it can look too early and miss the row; threading a waker through every writer would work, at the cost of a handle everywhere and a silent fallback when one is forgotten. **Decided: the SQLite relay polls**, about every 100ms in local mode — one author, one process, and a query on a small partial index. `FOR UPDATE SKIP LOCKED` falls away for the same reason — one writer, one relay, so claiming is an ordinary `UPDATE … WHERE … IN (SELECT … LIMIT n) RETURNING`.

**The cost is the schemas, and it is real.** Every feature's `migrations/` is PostgreSQL DDL and each needs a SQLite twin, drifting silently with no compiler to notice. The adapters themselves are ordinary work against conformance suites that already exist — event store, outbox, four catalogs, `PassageStore`, registry, deliveries — which is a lot of implementations but no new design. Dialect drift is mechanical: `TIMESTAMPTZ` becomes TEXT, `JSONB` becomes TEXT, `BIGSERIAL` becomes `INTEGER PRIMARY KEY`, and `COLLATE "C"` is free because SQLite's default collation is already byte order.

**What it does to M13:** export and import stop being the persistence mechanism and become what they always should have been — a way to move a project between machines. An author's work is durable because it is in a file that was written as they worked, not because they remembered to export.

### Milestone 12 — Ideas and passages ✅

**Goal:** the pool splits in two, and every view owns what it says about an idea.

**Decided 2026-10-01** and argued in full in [ARCHITECTURE.md](./ARCHITECTURE.md#ideas-and-passages-are-two-pools--decided). In short: a `Piece` is doing two jobs with different lifecycles — a disposable **idea** and a durable **passage** — and the complaint that attaching pieces to sections felt *"very weird"* was the model surfacing rather than an interaction problem.

**It goes before [local mode](#milestone-13--local-mode), and the reason is concrete rather than cautionary.** The first argument for doing it now was that reshaping three event-sourced features is cheapest while no author has durable data — true, but conditional on when a file reaches an author. The decisive one is simpler: **local mode means writing a SQLite adapter for every port we have** — event store, outbox, four catalogs, `PassageStore`, registry, deliveries — and a schema twin for each. Do that first and the pieces half of it gets written twice, because this milestone reshapes exactly those stores. Reshape first, port once.

**Build:**

- **`ideas`** replaces `pieces` as the pool: an id, a name, optionally a kind and a description. No passage link, no position, nothing a view could want to own.
- **`passages`** stays as it is — already a pool with its own ids and its own feature — and gains the optional idea that prompted it.
- **The outline arranges passages**, as the board arranges ideas, which keeps *the outline holds structure, never content* true rather than breaking it.
- **An appearances read model** (the `appearances` feature), fed by every view's events, answering *idea → where it appears*. It is what makes an inspector one read instead of five, and what lets a new view join by publishing rather than by anyone depending on it.
- **An inspector on the board** that can set a moment or a character without leaving the view — writing to those features, owning nothing. The board must stay usable with every inspector blank forever.

**The steps, each reviewable alone:**

1. **Rename `pieces` to `ideas`** — directory, crates, `PieceId` to `IdeaId`, `piece.*` routing keys to `idea.*`, the summaries table, the client. Pure vocabulary: 2,097 sites across 126 files and not one behaviour change, so it is reviewed by confirming nothing *but* names moved rather than by reading every hunk.
2. **A passage belongs to a project** — the project link it has never had, plus `in_project`, and the deletion cascade sweeps passages straight from `project.deleted`. Split in two: **2a** gives the passage its project, **2b** adds the sweep.
3. **The idea loses its passage** — `Idea.passage`, `PassageAttached`, the attach route and `Discarded { passage }` all go, and with them the `delete-passage-of-discarded-idea` listener.
4. **The outline arranges passages** rather than ideas. Split in two: **4a** the feature, **4b** the client. A leaf shows the passage’s opening words until [step 6](#milestone-12--ideas-and-passages) gives it a name, and a section gains **write here** — which is where a passage now comes from, since opening an idea used to be the only thing that made one.
5. **The outline also arranges ideas**, as notes beside the content — the first tagged reference. Split in two: **5a** the feature, **5b** the client. **The two kinds must be told apart on sight**: once a section can hold both, an idea and a passage drawn with the same glyph are indistinguishable, and an idea must not open an editor — clicking one goes to the idea. Flagged from use while the outline held only passages. The tray holds both, in two labelled groups — and the tally counts only prose, because a passage with no home is a to-do while a loose idea is the normal case.
6. **A passage has a title, and draws on any number of ideas** — both owned by the passage. Split as **6a** the title, **6b** the ideas, each server then client.
7. **The appearances read model** — and passages start announcing their links.
8. **The inspector, and the routes it rearranges** — in the client.

**Step 2 must precede step 3, and the reason is the nastiest failure in the list.** A `Passage` today is `{ id, doc }` — it has no project, and the only route from a project to its prose runs through the piece that links it. That is exactly what [the cascade](#milestone-11a--projects-event-sourced-and-the-deletion-cascade) exploits. Take the passage off the idea first and project deletion silently stops reaching the prose: no error, no dead letter, just a book that outlives everything that could find it.

**The listener is the whole point of step 3, not a loose end.** Everything else in that step is data; `delete-passage-of-discarded-idea` is the only *behaviour*, and it is the one piece of the codebase that contradicts the decision outright. It makes prose die with a scrap — and the argument for two pools was precisely that an idea is disposable while a passage is the book. A listener that turns throwing away a what-if into destroying a chapter is the coupling the split exists to remove. **Nothing replaces it.** Discarding an idea becomes free because it destroys nothing, and after step 2b the project sweep is the only thing that deletes prose.

**It could not have gone earlier, and it does not come back in step 6.** Before step 2b a passage had no project, so the idea's discard was the only route the cascade had to the prose — the listener was load-bearing, which is why M11a added `Discarded { passage }` to feed it. Step 6 then gives the passage its own link back to the idea that prompted it, and that link is **provenance, not ownership**: it says where the prose came from, carries no delete in either direction, and a discarded idea leaves it dangling rather than taking the passage with it.

**Step 6 was one link, and became two things.** It began as *a passage names the idea that prompted it* — one link, doubling as the leaf’s name. But a scene usually grows from several ideas at once, and with three behind a passage there is no answer to which one names it. So naming and provenance come apart:

- **6a — a passage has its own optional title**, edited on the passage’s own page, **below the editor**. That is what the outline shows, falling back to the opening words. *Write from this idea* may pre-fill it from the idea, as a **copy, never a reference**: renaming a scrap must not rename a scene, for the same reason a section carries its own title rather than borrowing an idea’s.
- **6b — a passage draws on any number of ideas**, a set owned by the passage, in its own table. It names nothing and deletes nothing. **A small passages listener removes a link when its idea is discarded** — pruning a reference, never touching text, so it does not bring back the coupling step 3 removed.

**Where a link is made.** Two places. On the passage’s own page, a **Drawn from** section under the editor lists the linked ideas — idea glyph, a link to the idea, × to unlink — with **Link an idea** opening a **dialog**, not a tray: linking is choosing from a list rather than placing something, a tray would sit beside the text for the whole session, and there is no drop target in prose. The dialog searches the project’s ideas, hides the ones already linked, and takes several at once. The second place is the inspector of [step 8](#milestone-12--ideas-and-passages), from the board.

**Step 7 needs passages to speak.** Boards and the outline publish every change, so a projection can follow them. Passages are not event-sourced and publish nothing about links — the only message they send is the sweep’s continuation. **Decided: passages announce `passage.idea.linked` and `passage.idea.unlinked`** through the publisher they already hold, so the appearances model treats every view alike and a new view joins by publishing rather than by being asked. The alternative — the inspector asking passages directly and merging the answer — was simpler, and made passages the one special case the read model exists to avoid.

**Which means steps 2 and 3 partly undo M11a.** `PieceEvent::Discarded { passage }` exists *because* a passage is unreachable except through its piece; give passages their own project and that carrying is dead weight. It was right for the model as it stood, and it is cheap to remove.

**Step 8 is bigger than "add a panel", because step 3 breaks a route.** Today `/projects/{p}/ideas/{idea}` opens the idea's prose: it reads the idea, follows `Idea.passage`, creates one if there is none, and hands you an editor. Every link into an idea in the client points there — the board chrome, the pool, the outline. Step 3 takes that link away, and with it the question that route answers: an idea no longer *has* a passage, it may touch none or five, so there is nothing singular left to open.

So the inspector is not a new surface beside the old one, it is what that route becomes:

- **`/projects/{p}/ideas/{idea}` becomes the inspector** — the idea's name, and its appearances: the sections that note it and the passages that draw on it — and, once those views exist, the moments and characters it is tied to. The board is deliberately not among them: it is where ideas are thought about, not where they are used. One read, from step 7's read model.
- **The prose gets its own route**, `/projects/{p}/passages/{passage}`, reached from the outline and from the inspector's passage appearances. The editor itself does not change; only who links to it does.
- **The panel on the board is the same inspector, docked**, filled from the existing selection.

**Step 8 in two reviews.** **8a** — the idea's page becomes the inspector: its name edited in place as the page's heading, and *Appears in* listing the sections and passages, their titles joined in the client from the outline and the project's passages. **8b** — the same inspector docked on the board, following selection.

**The inspector follows selection, not a double-click.** Double-click on a card already means rename in place, and selection already exists (`handles.selected`, set on `focusin`). Hanging the inspector off selection costs no new gesture and takes nothing away: single click inspects, double-click still renames, and the inspector's name field is that same edit rather than a second way to do it. Double-click would have to displace rename to a worse home for no gain.

**Deliberately out of scope:** a `kind` and a `description` on an idea — both additive, neither needed to prove the model — and anything timeline-shaped, which is what the twenty-ideas spike should inform first.

**Later: passages and ideas reorderable within their group, the way sections are.** The model already allows it — `Attach` takes an `after` and detaches first, so attaching to the same section behind a different neighbour *is* a reorder, with no new command or event. What's missing is purely the client: a grip on a leaf, landing between leaves, and keyboard moves (`Alt+↑/↓`) to match the sections. **Within its group, never across** — a section shows its passages and its ideas as two groups, and a note belongs to its section rather than to a place between two scenes, so an idea is reordered among ideas and a passage among passages.

**Later: the passage page as two columns.** Writing comes first and must be possible without distraction, so the passage’s metadata — its title, the ideas it draws on, whatever joins them — moves out from under the editor into a **collapsible sidebar** on the right. Step 6 puts it below the editor as a stopgap; this is where it ends up when the editor is redesigned.

**Later: a paged, searched link dialog.** The dialog on the passage page loads every idea in the project and filters in the browser — fine for a book, not for 10k+ ideas. When that bites: a server-side route that searches by title and pages with a cursor, the dialog asking it as the author types. The cursor half already exists — the ideas catalog’s `in_project_after` is what the project sweep pages with; searching by title is the new part, and wants an index of its own (prefix or trigram) rather than a scan.

**Later: rework the wiring** — done as [M13 step 0b](#milestone-13--local-mode). It has grown unwieldy again. Each feature is named in `services/api` some ten times over — its `Ports` in-memory and on PostgreSQL, its database, its `lay_out`, its `wire`, and its outbox in `outboxes()` — and most of those must be remembered by hand: leaving a feature out of `outboxes()` compiles cleanly and silently stops its messages, which only an end-to-end test catches. And `publisher` and `clock` are threaded through every `Ports` constructor separately. The direction worth trying: each feature hands the service **one value** that knows its ports, schema, outbox and routes, and the service iterates a list instead of naming every feature at every step — so a new feature is one line, and forgetting a part of it stops being possible.

**Later, with authorization: messages name their project, and appearances store it.** Every published message carries an aggregate's `{ id, kind, version }` and nothing about which project it belongs to, so a projection cannot scope by project without a lookup of its own. Authorization will force the change anyway — once requests are scoped to projects, so is everything announced about them — and the right shape is the envelope, once, in `eventpublishing`, rather than each feature adding it to its payloads. Then [appearances](./features/appearances) gains a `project_id` column: project deletion becomes one `DELETE` on `project.deleted` instead of riding the idea sweep, and reads are scoped to a project, so an id from another project answers nothing. Not before: correctness does not need it today, and a lookup table kept only for this — or a project on some rows and not others — is worse than waiting.

**Later: controls for the tree as a whole** — collapse everything, expand everything, and whatever else turns out to be worth one click rather than a click per section. Client-only by construction: folding is a `RwSignal` in the view, and it has to stay there — expansion state in the event stream is named in [ARCHITECTURE.md](./ARCHITECTURE.md#the-outline-arranges-sections-not-pieces) as the structural failure of tree-as-model. Whether the fold should survive a reload is a separate question, and the answer there is browser storage, never the aggregate.

**Done when:** an idea carries no link to any view; a view gains a new kind of relation without `ideas` changing; the appearances read model answers in one request; and the board is usable without ever opening an inspector.

*Done. `Idea` is `{ project, title, discarded }` and nothing else, and the proof of the second criterion is in the history: `features/ideas` has not changed since step 3, while the outline learned to note ideas (5), passages to link them (6b) and appearances to follow both (7). Every link is owned by the side that makes it — the outline's `Attachment`, the passage's `passage_ideas` — and each announces it through its own outbox; passages, not event-sourced, learned to announce in the same transaction as the change. Appearances store `Subject` at `Place` generically and stay typed in the domain, with boards deliberately not places. The client's `/ideas/{idea}` became the inspector, and the board docks the same `InspectorState` at the top of its tray, following selection, with renames meeting in the middle by version. Along the way: `Detached` names the section it left, the outline catalog's columns became `attachment_type` / `attachment_id`, the test fixture closes its pools and takes turns, and a client view keeps its state in a `<View>State` ([CONVENTIONS.md](./CONVENTIONS.md#a-type-is-named-for-what-it-is-spelled-out)). Left for later, as listed above: reordering within a group, tree controls, the two-column passage page, a paged link dialog and project ids on messages; the wiring rework moved to M13.*

**Still unknown, and deliberately not gating this:** how often an idea maps one-to-one onto a passage, and what the timeline wants to hold. Those are ergonomics and they shape the *views*; the structural question was settled on lifecycle. Twenty real ideas on a board will answer them, and that is worth doing before the timeline is designed rather than before this.

---

### Milestone 13 — Local mode

**Goal:** an author runs Weaveling on their own machine with no database server and no broker, and their work lives in files they own — written as they work, so it survives a restart.

Sits beside M11 on purpose: *"the real store"* and *"no store at all"* are two answers to the same question, and the ports mean neither has to win.

**Backed by SQLite** — see [the decision](#sqlite-backs-local-mode--decided): one file per feature plus one for messaging, the relay woken by a `tokio::sync::Notify` rather than `LISTEN/NOTIFY`, and a schema twin for every migration.

**One binary, the backend chosen at runtime.** Decided 2026-10-07. Every backend is compiled in and configuration picks: `WEAVELING_DATABASE_URL` → PostgreSQL, a data directory → local SQLite, neither → in memory. A third backend does not fit compile-time features — they stop being additive, and `--all-features`, which CI builds, would need a precedence rule — and one binary for every mode is what an author downloads anyway.

**Export moved out, to [M13a](#milestone-13a--moving-a-project-between-machines).** It was here as the only thing standing between an author and a lost afternoon. With SQLite, work is durable because it was written as it happened, and export becomes what it should always have been: a way to move a project between machines.

**Local is single-user by definition** — no accounts, no authors on other machines. Two tabs on the same computer still collaborate, because the sync socket is local and knows nothing about deployment.

**It follows [M12](#milestone-12--ideas-and-passages) on purpose**, because local mode writes a SQLite adapter for every port in the codebase and M12 reshaped several of them. Porting first would have meant porting the same stores twice.

**The steps, each reviewable alone:**

0. **What has to be true first.**
   - **0a — passages are evicted, and compacted on the way out.** The [open TODO](./TODO.md) with its intended shape: participants counted inside the map's write lock, a sweeper that collects entries idle for a grace period, and a final persist before anything leaves memory. A process that lives as long as an author's afternoon has no restart to save it.
   - **0b — the wiring is reworked.** Moved here from M12's *later*: each feature hands the service one value that knows its ports, schemas, outbox and routes. Without it a third backend means some ten more edits per feature in `services/api`, and forgetting an outbox keeps compiling.
1. **Groundwork.** Every migration moves into `migrations/postgres/`, with its `migrations/sqlite/` twin arriving beside it in the step that ports that adapter; sqlx gains its `sqlite` feature, the test harness a `SqliteFixture`, and `wiring::sqlite` lays out one file per feature — while `wiring::database` becomes `wiring::postgres`, since there are now two. No adapter yet.
2. **The event store and outbox on SQLite.** **2a** events and snapshots — and `enqueue`, since an append writes its messages in the same transaction. Between the two, **the outbox moved out of `eventsourcing` into its own library**: passages announce through it without being event-sourced, and should not need an event store, or an `events` table, to do so. **2b** the SQLite outbox and its relay, in that library. Claiming is an ordinary `UPDATE … RETURNING`, since one process means one relay.
3. **Deliveries and the registry on SQLite.** Retries and dead letters must survive a crash, or the [durable delivery](#milestone-11b--one-flow-in-every-mode) is a lie in local mode.
4. **The five catalogs on SQLite** — projects, ideas, boards, outline, appearances. Mechanical against existing suites; one commit each, one review.
5. **The passages store on SQLite** — updates, titles, linked ideas.
6. **The service runs locally.** Split in three: **6a** `Storage::Sqlite` and every feature on it, proven by the service tests ported to SQLite and a restart from files alone; **6b** the binary choosing at startup — `WEAVELING_DATABASE_URL` for PostgreSQL, `WEAVELING_DATA` for a data directory, neither for memory, both refused — with every backend compiled in and the local relays polling every 100ms, since the SQLite outbox is never notified; **6c** the browser suite run against local mode as well as in memory.
7. **A refused message reaches the author.** Retries and a durable dead-letter table exist since M11b; in local mode there is no ops staff to read the table, so the client shows what was refused and why.

**Done when:** an author can work with no database server running, stop the process, start it again, and find their ideas, board, outline and passages exactly as they left them; a refused message is shown to them rather than only stored; and the browser suite passes against local mode.

**Later: revisit the wiring again.** Step 0b is good enough to build local mode on, not where it should end. Candidates noticed on the way: every feature now spells its wiring three times over — `Ports`, the free `wire` and `service` its tests use, and the `Feature` impl that mostly delegates to them; each backend adds a `#[cfg]` pair of trait methods with defaults, so the impls grow per backend; and `Storage` is an enum the trait has to keep in step with. Worth a fresh look once SQLite has shown what a third backend really costs.

**Later: two executables, one per audience.** What M13 ends with is one binary that picks its backend from the environment, which is right for us starting it from a terminal and wrong for an author. The target is two executables over the same library:

- **A local one for authors** — SQLite, and local mode without being told: when nothing is configured, the work goes to the platform's data directory (`%APPDATA%\Weaveling`, `~/Library/Application Support/Weaveling`, `~/.local/share/weaveling`). A `--data <directory>` flag moves it, falling back to `WEAVELING_DATA` — `clap` with its `env` support gives both from one declaration, and `--help` makes it discoverable.
- **A server one** — PostgreSQL, configured by `WEAVELING_DATABASE_URL` as now, since containers and hosting platforms configure by environment and it keeps a connection string out of the process list.

In memory stays what development and the tests run on. `Backend::chosen` survives as the shared core; each executable only decides its default and where its settings come from. This revisits *one binary, the backend chosen at runtime* from the top of this milestone: the runtime choice stays, but the binary each audience runs is narrowed to what that audience needs.

---

### Milestone 13a — Moving a project between machines

**Goal:** a project leaves one installation as a file and arrives in another whole.

**Build:** export a project — the projects rows, every aggregate's event stream with metadata, each passage's CRDT state via `Passage::everything()` and its links — and import it. **Catalogs are not exported**, deliberately: import replays each stream in the file through its projector, so a read model is rebuilt rather than restored, and the file's contents are the enumeration the event store cannot give. That makes import the first wholesale rebuild of a projection, which the [open TODO on enumeration](./TODO.md) notes is otherwise missing.

**Done when:** a project exported from one instance and imported into an empty one has its ideas, board, outline and passages exactly as they were — with every catalog rebuilt by replay.
---

### Milestone 14 — Two languages

**Goal:** German and English, chosen by the reader rather than the build.

**Build:** every string in the client comes out of the markup and into a catalogue, with a language the author picks and the browser's preference as the default. Dates and times already go through `time` and will need the locale too.

**Know before starting:** **the E2E suite selects by English accessible name** — `getByRole("button", { name: "Pin The loom remembers" })`, across 107 tests. Either every spec pins a locale, or the selectors move to test ids and lose the accessibility check they currently double as. That decision is most of the work's character, so make it first.

**Done when:** the whole app reads in German, the choice survives a reload, and the suite still passes in both.

### Milestone 15 — Touch and small screens

**Goal:** the app in a pocket, for the edits that happen away from a desk.

**Build:** the writing view and the workspace are a media query away — they are already a single column. **The board is not.** Pointer events already carry touch, but the resize grips are 7px where a finger needs about 44, `touch-action: none` on the corkboard means the browser will not help, and **pinch-to-zoom is not wired at all** — ctrl-and-wheel is the only zoom, and pinch is *the* gesture on a phone. That is a feature, not a stylesheet.

**Done when:** a piece can be captured, opened and written on a phone; the board pans and pinch-zooms with two fingers; and a card can be moved with a thumb.

## After Phase 3 (sketch only)

- **Accounts, tenancy and auth** — looming. It changes port shapes (`list(owner)`, not `list()`) and puts auth on the sync socket. Not a filter to bolt on.
- **Export** — Typst → PDF and HTML → EPUB. Still an **unspiked risk**: embedding Typst means implementing its `World` trait for font and file resolution, plus font licensing. Worth its own rung before it becomes a milestone.
- **A board that holds ten thousand pieces** — three related changes, and the order matters because the first probably makes the second unnecessary.

  **Cull to the viewport first.** Today every placement becomes an `<article>` with a real `<a>` inside; at ten thousand that is twenty thousand nodes for Leptos to diff and the browser to lay out. But at any zoom where a card is *readable*, only tens of them fit on screen — ten thousand cards at 150px is over a kilometre of board. So rendering only what is in view is both the cheapest fix and very likely the sufficient one.

  **Canvas is the tempting answer and it costs more than it looks.** One element and draw calls would be fast, and it would break two decisions this codebase made deliberately: cards are real anchors, because [`<A>` over a click handler](./ARCHITECTURE.md#frontend--full-stack-rust) is what makes a piece ctrl-clickable and its link copyable; and the E2E suite selects by **role and accessible name**, which a canvas is opaque to. A canvas would take the links, the screen reader and the test strategy with it. The honest split is by zoom: DOM plus culling while cards are readable, and canvas only for a zoomed-out overview where titles cannot be read anyway — and where losing the link therefore costs nothing.

  **Loading per viewport is the interesting one, and it pressures the frontend join.** Spots are coordinates, so `within=x1,y1,x2,y2` is a natural query — but it cannot be served from the board aggregate, which holds every placement in one stream, so it wants a region-keyed projection. Worse, windowing the placements alone buys nothing: the client resolves titles by fetching *the whole pool*, which at ten thousand pieces is the larger payload. Either both sides get windowed together, or the board's read model carries the title — and that second option denormalises a piece's title into `boards` and needs a listener on `piece.retitled`, which is exactly the [client-side join](./ARCHITECTURE.md#the-board-renders-through-a-frontend-join) we chose instead. So the real question is not "add a spatial query" but **whether the board keeps joining client-side once the pool is too big to fetch whole.**

- **Test readers** — sending a sample out for comment, and getting a review back. The shape the author wants: **one link per reader**, each carrying a token, so a single reader's access can be revoked without touching anyone else's. The link opens a **sample** — a subset of the outline, or the whole book. The reader sees the outline as a table of contents alongside the text, and leaves comments on a **range of text, a paragraph, or a whole section**. When they *commit* their review it becomes visible to the author, who works through it.

  **When: the reading view is a strong candidate as soon as the board and the outline are finished and both carry their live channel** — after M10 and [M9b](#milestone-9b--the-boards-live-channel), and after whatever M9b turns out to be for the outline. It sits in this section by topic rather than by date, the same way M9b does.

  It can come that early because **the two halves have very different prerequisites.** Reading your own book needs no capability model at all — no tokens, no accounts, nothing that leaves the machine — so the reading view is buildable long before the auth work that sending a sample out would demand. And it earns its place right after the structure work rather than in spite of it: finishing the outline is the moment the author most needs to see whether the structure actually *reads*, and until then they have only ever seen the book as parts. The token, the sample and the review lifecycle are the second half, and they wait for accounts and tenancy.

  **The strong half of the idea is that this is a reading view, not a separate product.** The alternative — a second micro frontend for outsiders — builds a whole reader that the author never uses. One view with two roles gives the author something they would otherwise have to ask for separately: a way to read their own book end to end and leave comments on it like anyone else. Build the reading view; a test reader is that view with a token and no edit rights.

  **It is the first time anything leaves the author's hands, and that is the real cost.** [Ids are not capabilities](./TODO.md) today — `/sync/{passage}` accepts anyone who names a passage that exists, and the REST layer has the same tenancy gap. A per-reader revocable token *is* a capability model, and it would be this app's first. Smaller than accounts and tenancy, pointing the same way, and probably the thing that forces that milestone rather than the other way round.

  **A sample is frozen — decided.** The author keeps writing while readers read, and a live sample would mean prose shifting under someone mid-chapter and feedback arriving about sentences that no longer exist. So a sample is pinned to a moment: the outline read through to a version, the piece titles as they stood, and the prose as it was.

  **Freezing the prose means copying it, and the copy should be a Yjs state rather than exported text.** Two reasons, and the second is the interesting one. First, reconstructing a passage as it *was* is not free — Yjs can restore a prior state only with garbage collection disabled, which costs memory for the lifetime of every document, and [compaction is already owed](./TODO.md) precisely to stop the log growing. Copying the bytes at freeze time is simpler, self-contained, and means deleting a sample deletes its prose with it. Second: because the frozen copy is derived from the same document, it shares item identity with the live one, so **a comment anchored by relative position in the sample still resolves in the manuscript the author is writing now.** Exporting to text would sever that and leave the author matching quotes by hand. Freezing does not remove the mapping problem — it moves it to the one place it can be solved.

  Sending a revised chapter is then a new sample rather than an edit to an old one, which is also how an author would describe it.

  **A review is its own aggregate**, one per reader per sample, with a lifecycle: comments accumulate privately, then commit makes them visible. That draft-then-submit shape is why it cannot just be comments hanging off passages. Section-level comments anchor to a `SectionId` and are easy; paragraph and range anchoring is where the work is. And a reader needs a name the author recognises, which probably means the author names the link when they make it, rather than the reader introducing themselves.

- **Write an actual book in it, and find out whether any of this helps.** Everything so far rests on a suspicion: that a pool of pieces, a board and an outline are what an author actually needs. Nobody has tested that, and no amount of green suites will. The intended first test is a **children's story** — a girl in the early medieval period, with the turn of a fable or a fairy tale, and possibly a princess. Small enough to finish, structured enough to exercise parts, chapters and scenes.

  **It is too early, and the reason is specific.** The writing view is not finished and there is no reading view at all, so an author could capture and arrange a book without ever being able to read it back — which is exactly the part the test needs to judge. That puts this after [the reading view](#after-phase-3-sketch-only), which is why the two belong together: the reading view is what makes the test possible, and the test is what tells us whether the reading view was the right thing to build.

  **What to watch for when it happens** is not bugs but silences — the moment the author stops using the board, or keeps a list somewhere outside the app, or cannot find a scene they know they wrote. Those say more about whether the model is right than any feature request would.

- **An assistant, reached over MCP rather than built in.** The author wants an LLM for consistency checking (does this dialogue actually belong to this character?), planning and brainstorming. The instinct not to call a paid API from inside Weaveling is right, and for a stronger reason than cost: a local-first tool that needs a vendor key to be useful is not local-first. Exposing Weaveling as an **MCP server** puts the key, the model choice and the bill where they belong — with the author, in whichever assistant they already use.

  **It is not another client; it is a fourth adapter.** That is the whole reason it is cheap. `features/*/adapters/mcp` would sit beside `rest`, `catalog` and `messaging`, over the same services, exactly as [the dependency rules already allow](./ARCHITECTURE.md#dependency-rules) — a manuscript-shaped tool surface (`list_pieces`, `read_passage`, `outline`, `search`) rather than a second application. Calling it a client makes it sound like the work is a UI; it isn't.

  **Drop spelling from the list.** It is the weakest item there by a distance: a dictionary does it better, offline, instantly and for nothing, and spending an LLM round trip per typo is the wrong tool. What an assistant is uniquely good at is the things a dictionary cannot see — a character who was described as left-handed in chapter two, a scene that contradicts a timeline, a thread that was dropped. Those need the whole book, which is the argument for tools over prompts.

  **The tools want to be search-and-slice shaped**, because a novel does not fit in a context window. An agentic client pulling the three scenes it needs beats the app stuffing a manuscript into a prompt — which makes the [search projection](#after-phase-3-sketch-only) considerably more valuable than it looks today. It is the tool the assistant would lean on hardest.

  **Reads first; writes are a separate decision.** "Create pieces for these five ideas" and "reorder the outline" are the useful ones and also the frightening ones. Event sourcing is the right substrate for that — every change is already attributed and reversible — and `Agent` is `Anonymous | System | User(AgentId)` today, so an `Assistant(AgentId)` variant would make the audit log say plainly which changes an author made and which a model suggested. That distinction should exist *before* the first write tool does, not after.

  **`Assistant` is not `System`, and the two must not be collapsed.** `System` is the application acting on its own — a projector, a migration, a scheduled sweep — with no human behind it. An assistant acts *on the author's behalf*, at their prompting, and its changes are ones the author may want to review, attribute or undo as a group. Reusing `System` would make those changes indistinguishable from the app's own bookkeeping, which is exactly the question the log would be asked. The name should say the role rather than the technology, too: `Assistant`, not `Ai` — the log records who acted, and "AI" describes what it is built from.

- Then, in some order: the codex, the timeline, threads, search projections over Tantivy.
