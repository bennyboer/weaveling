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

**It is blocked on a decision, not on effort.** `projects` is the last feature that is not event-sourced — it is Phase 1 CRUD over a `ProjectStore` — so there is no `project.deleted` on the wire to listen to. Getting one means either event-sourcing `projects` properly (aggregate, catalog, projector, `If-Match` — the whole M7 treatment) or hand-rolling a publish inside `ProjectService::delete`, which would be a second publishing path bypassing `eventpublishing` that we would delete again later.

**And its payoff is still theoretical.** An orphaned piece from a deleted project is unreachable — you cannot navigate to a project that no longer exists — and dies at process restart. It becomes real at [M12](#milestone-12--local-mode), where in-memory *is* the store, which is also about when event-sourcing `projects` starts paying for itself: **deletion is exactly where an author wants an audit log.** The two belong together, so they now live together in [M11a](#milestone-11a--projects-event-sourced-and-the-deletion-cascade).

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

**Not next.** [M9a](#milestone-9a--the-shell) is, then M10; this sits here because it belongs to the board, not because of when it will be built.

**Goal:** two browsers on one board, seeing each other work.

**Build:** a second live surface. Awareness is bound to `/sync/{passage}` today and a board is not a passage, so the board needs its own socket carrying two different kinds of traffic: **committed events**, which are already published (`board.#`) and want a `Delivery::Fleeting` subscriber rather than a durable queue, and **in-flight drags**, which are awareness — a card travelling under someone else's pointer, never written down. The earlier implementation had precedent worth copying: a WebSocket pushing event messages, with per-event handlers patching a local store on the client.

**Rust you'll meet:** a second WebSocket surface that is deliberately *not* a CRDT, and awareness carrying something other than a cursor.

**Done when:** two browsers drag pieces on one board and see each other live; the durable record still holds one event per drop rather than per frame; and a peer that drops out leaves no ghost card mid-drag.

**Know before starting.** Two things are already recorded and both bite here. `InProcessDispatcher` delivers synchronously, so it will **hide** the very races this exists to expose — the board's projection never lags in a test, and the find-or-start race stays invisible. And the client's board state gains a second writer: every optimistic write, rollback and version guard in `open_board.rs` was written assuming this author is the only one moving cards. The `Reshape` command carrying only what changed was chosen with exactly this in mind, and it gets its first real test here.

**Explicitly not in M9a:** presence beyond the board, cursors in prose (that is the passage socket's job, already built), and any attempt to make drags durable.

### Milestone 10 — The outline

**Next.**

**Goal:** manuscript order, as a view over the pool rather than a property of it.

**Build:** `features/outline` — **sections** that nest and hold pieces, rather than nesting the pieces themselves. That was decided after the milestone was written, and it changed the shape: a section carries its own title, because a table of contents rarely wants the working title of the idea a scene grew from, and because the structure of a book is not the structure of the ideas it came from. See [The outline arranges sections, not pieces](./ARCHITECTURE.md#the-outline-arranges-sections-not-pieces) for the full reasoning and the guardrails.

Moves are shaped like an author's intent (*move*, *promote*, *demote*) rather than the "swap two nodes" primitive the earlier implementation was cornered into by its data structure. **Promote takes the sections that followed it along as children**, because the alternative silently reorders the book.

**The whole back end is done** — `features/outline` in the same five-crate shape as `boards`: core (aggregate, catalog port, service), contract, and adapters for catalog, messaging and REST, wired into the composition root. 92 tests across the feature. The aggregate covers both orderings, the promote-adoption rule, subtree moves, the cycle refusal, removal lifting children into place and a snapshot round trip; the integration tests drive all of it through HTTP, and one of them replays the log through a **service that never saw the writes**, so the structure and the reading order have to come back out of the events rather than out of whoever wrote them.

**One projection subtlety worth remembering:** the attachment index has to wake on `SECTION_REMOVED` as well as attach and detach, because removing a section returns its pieces to the pool. Listening only to the two obvious events leaves `outlines_holding` claiming pieces the book no longer contains — and the discard cascade reads that index.

**Still owed: the client view.**

This is the privileged view: export needs a linear order, so the outline is what "the manuscript" means. A piece may sit on the board and be absent from the outline — it simply is not in the book yet.

**Still open:** the split-piece mechanics — see the design threads in [TODO.md](./TODO.md).

**Not in M10:** undo/redo. The event stream makes it available whenever it is wanted, which is exactly why it does not need to be built alongside the outline.

**Done when:** a book-shaped outline of chapters and scenes, each openable in the editor, structural changes visible in the audit log, and rebuilding the projection from scratch reproducing the same order.

### Milestone 10b — The outline's live channel

**After [M9b](#milestone-9b--the-boards-live-channel), deliberately.** The board's channel is the harder one to design — free placement, drags emitting thousands of frames a second — and whatever it settles about transport, awareness and a second live surface, the outline reuses rather than re-decides.

**Goal:** two browsers on one outline, seeing each other restructure.

**But the conflict story is the opposite of the board's, and that is the whole milestone.** [Moves of different pieces on a board commute](./ARCHITECTURE.md#the-event-catalogue), which is why `PieceMoved` takes no strict version check and concurrent drags are last-drop-wins — no work is lost, the card simply lands where the last author dropped it. **Tree moves do not commute.** Two authors moving sections at once can produce a cycle, or leave a section parented to one that has just been removed, and "last writer wins" on a tree can silently discard a whole subtree's placement. A position is safe to overwrite; a structure is not.

**This is where the intent-shaped commands earn their keep**, and they were chosen partly for it. A client-computed `Move { under, after }` is a placement derived from a view that may already be stale, so under concurrent editing `NoSuchNeighbour` starts firing in earnest. `Promote` and `Demote` carry only a section id and compute the placement from state at the moment they are applied, so they cannot go stale — which is the same reason [`Reshape` carries only what changed](#milestone-9--the-board-). Expect the [retry on version conflict](./ARCHITECTURE.md) to matter far more here than it does on the board.

**One piece of view state is neither durable nor shared:** which twisties are open. It is not in the aggregate — [that was the rejected design's structural failure](./ARCHITECTURE.md#the-tree-is-a-view-not-the-model) — and it does not belong in awareness either, because nobody wants their outline folding itself to match a collaborator's. Local only, and worth saying out loud because "not durable" and "therefore awareness" is the easy wrong step.

**Done when:** two browsers restructure one outline and see each other do it; no sequence of concurrent moves can produce a cycle or orphan a subtree; and the durable record still holds one event per move rather than per frame.

### Milestone 11 — The real store

**Goal:** prove the abstractions were worth the trouble.

**The database choice is reopened.** PostgreSQL was parked early; MongoDB is under consideration again. The decision belongs here, judged by the conformance suites rather than by preference, and it changes the outbox mechanics (`LISTEN/NOTIFY` versus change streams) without touching anything above a port.

**Build:** a second backend for every port that has one — `ProjectStore`, `PassageStore`, the event store — as modules behind an optional cargo feature, not sibling crates. CI must run `--all-features` or none of it is type-checked.

**This is where storage representation finally gets decided,** and where the transaction tests that in-memory cannot express have to be written: rollback, connection failure mapping to `StoreError::Backend`, and the concurrency guard `apply` needs if `PassageStore` goes the snapshot route. The event store's `append` must be atomic across the version check, the append **and** the outbox insert — one transaction, invisible above the port.

**Done when:** the suites pass unchanged against the real store, and swapping backends is one line in one manifest.

### Milestone 11a — Projects event-sourced, and the deletion cascade

**Goal:** the last CRUD feature joins the rest, and deleting a project actually leaves nothing behind.

Deferred out of [M8](#milestone-8--messaging-), where the cascade was originally scoped. Sits here because both halves start paying at the same moment: with a durable store — and certainly in [local mode](#milestone-12--local-mode) — an orphan outlives the session, and **deletion is exactly where an author wants an audit log**.

**Build:** `Project` as an aggregate — `Started`, `Renamed`, `Deleted` — with the catalog, projector and `If-Match` handling that `pieces` already has, so the codebase stops carrying two shapes of feature. Then the cascade itself: deleting a project disposes its pieces, boards and passages, tolerating a project that never had a board.

**The open question is choreography or orchestration.** Each feature listening for `project.deleted` and disposing its own is far simpler and is the right default; a saga with its own state earns its place only if the cascade needs ordering, compensation, or a completion signal an author can see. Decide it against the real requirement rather than in advance — but note that "recoverable and observable when it fails midway" leans toward orchestration, and that a saga that entails another (a `BoardDeletion` inside a `ProjectDeletion`) is a question the `Conversation` id was designed to answer.

**Done when:** deleting a project leaves nothing behind; a cascade that fails midway is recoverable and observable; the project's own history says who deleted it and when; and `projects` looks like every other feature.

---

### Milestone 12 — Local mode

**Goal:** an author runs Weaveling on their own machine with no database and no broker, and their project is a file they own.

Sits beside M11 on purpose: *"the real store"* and *"no store at all"* are two answers to the same question, and the ports mean neither has to win.

**Build:** export a project — the projects rows, every aggregate's event stream with metadata, and each passage's CRDT state via `Passage::everything()` — and import it at startup. The **piece catalog is not exported** — a deliberate choice, not a constraint — so import replays each stream in the file through the projector. That needs no new store capability: the enumeration is the file's contents. Plus a strict `InProcessDispatcher` that surfaces a refused message to the author instead of dead-lettering it for an ops team that does not exist.

**Local is single-user by definition** — no accounts, no authors on other machines. Two tabs on the same computer still collaborate, because the sync socket is local and knows nothing about deployment.

**Depends on** the deferrals that in-memory made free being closed first, since a session now lasts an afternoon rather than a test run: the catalog's dual write, passages never being evicted, and the deletion cascade. See [TODO.md](./TODO.md).

**Done when:** an author can work with no database running, export the project, restart with an empty process, import, and find their pieces and prose exactly as they left them — with the catalog rebuilt rather than restored.

---

### Milestone 13 — Two languages

**Goal:** German and English, chosen by the reader rather than the build.

**Build:** every string in the client comes out of the markup and into a catalogue, with a language the author picks and the browser's preference as the default. Dates and times already go through `time` and will need the locale too.

**Know before starting:** **the E2E suite selects by English accessible name** — `getByRole("button", { name: "Pin The loom remembers" })`, across 107 tests. Either every spec pins a locale, or the selectors move to test ids and lose the accessibility check they currently double as. That decision is most of the work's character, so make it first.

**Done when:** the whole app reads in German, the choice survives a reload, and the suite still passes in both.

### Milestone 14 — Touch and small screens

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

- **An assistant, reached over MCP rather than built in.** The author wants an LLM for consistency checking (does this dialogue actually belong to this character?), planning and brainstorming. The instinct not to call a paid API from inside Weaveling is right, and for a stronger reason than cost: a local-first tool that needs a vendor key to be useful is not local-first. Exposing Weaveling as an **MCP server** puts the key, the model choice and the bill where they belong — with the author, in whichever assistant they already use.

  **It is not another client; it is a fourth adapter.** That is the whole reason it is cheap. `features/*/adapters/mcp` would sit beside `rest`, `catalog` and `messaging`, over the same services, exactly as [the dependency rules already allow](./ARCHITECTURE.md#dependency-rules) — a manuscript-shaped tool surface (`list_pieces`, `read_passage`, `outline`, `search`) rather than a second application. Calling it a client makes it sound like the work is a UI; it isn't.

  **Drop spelling from the list.** It is the weakest item there by a distance: a dictionary does it better, offline, instantly and for nothing, and spending an LLM round trip per typo is the wrong tool. What an assistant is uniquely good at is the things a dictionary cannot see — a character who was described as left-handed in chapter two, a scene that contradicts a timeline, a thread that was dropped. Those need the whole book, which is the argument for tools over prompts.

  **The tools want to be search-and-slice shaped**, because a novel does not fit in a context window. An agentic client pulling the three scenes it needs beats the app stuffing a manuscript into a prompt — which makes the [search projection](#after-phase-3-sketch-only) considerably more valuable than it looks today. It is the tool the assistant would lean on hardest.

  **Reads first; writes are a separate decision.** "Create pieces for these five ideas" and "reorder the outline" are the useful ones and also the frightening ones. Event sourcing is the right substrate for that — every change is already attributed and reversible — and `Agent` is `Anonymous | System | User(AgentId)` today, so an `Assistant(AgentId)` variant would make the audit log say plainly which changes an author made and which a model suggested. That distinction should exist *before* the first write tool does, not after.

  **`Assistant` is not `System`, and the two must not be collapsed.** `System` is the application acting on its own — a projector, a migration, a scheduled sweep — with no human behind it. An assistant acts *on the author's behalf*, at their prompting, and its changes are ones the author may want to review, attribute or undo as a group. Reusing `System` would make those changes indistinguishable from the app's own bookkeeping, which is exactly the question the log would be asked. The name should say the role rather than the technology, too: `Assistant`, not `Ai` — the log records who acted, and "AI" describes what it is built from.

- Then, in some order: the codex, the timeline, threads, search projections over Tantivy.
