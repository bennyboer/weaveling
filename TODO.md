# Weaveling — TODO

Scratch notes. See [README.md](./README.md) (the dream), [ARCHITECTURE.md](./ARCHITECTURE.md) (the how) and [ROADMAP.md](./ROADMAP.md) (the plan) for settled decisions.

## Next up

- [x] **Build a proper roadmap.** Done — see [ROADMAP.md](./ROADMAP.md). Phase 1 is a projects-only CRUD prototype: in-memory store behind a trait, no Postgres, no event sourcing.
- [x] Milestone 0 — workspace scaffolded, builds green, health endpoint live.
- [x] Milestone 1 — `ProjectId`, `ProjectName`, `Project` in `features/projects/core`.
- [x] Milestone 2 — `ProjectStore` port, `StoreError`, in-memory adapter, conformance suite.
- [x] Milestone 3 — `ProjectService`, `contract` DTOs, REST adapter, composition root.
- [x] Milestone 4 — the Leptos client: list, create, rename, delete. Verified in a browser.
- [x] Milestone 5 — release bundle measured. Everything else in M5 deferred, see below.

**Phase 1 is done.** A projects-only CRUD prototype runs end to end: Leptos client → REST → service → in-memory store. 61 tests.

**Phase 2 spikes are done.** The prose stack is de-risked across three spikes — `spikes/crdt` (CRDT semantics, `yrs` ↔ `Yjs`, reading a `y-prosemirror` document), `spikes/editor` (Leptos hosting ProseMirror, two replicas converging across a partition), `spikes/sync` (real `y-websocket` clients through our own Rust server). Findings live in [ARCHITECTURE.md](./ARCHITECTURE.md#prose--the-editing-stack).

## Carried out of the Phase 2 spikes

Real gaps found while spiking, to settle when prose becomes production code rather than a spike.

- [ ] **Awareness tombstones on disconnect.** The sync server treats awareness as opaque bytes, which is the right default — but it means it cannot retract a departed peer's cursor, so stale cursors linger until the client-side timeout (~30 s). Fix: decode just the awareness header (client id, clock, state), remember which ids a connection spoke for, and broadcast a null state when it drops.
- [ ] **An unusable frame currently only logs.** A peer that sends an update we cannot apply keeps its connection and diverges silently. Decide the policy: disconnect, or force a full resync.
- [ ] **Compaction has to actually run.** Measured: an append-only log is 11–13× the compacted form after 500 rewrites of one paragraph. `Y.mergeUpdates` is lossless so this is safe, but nothing schedules it yet. Natural trigger: eviction (below), so a passage is compacted on its way out of memory.
- [ ] **Live passages are never evicted.** `LivePassages::hydrate` inserts into the `Hydrated` map and *nothing ever removes*. On disconnect `stay()` aborts its tasks and drops its channel; the map is untouched. So a passage stays in memory for the process lifetime whether anyone is connected or not — bounded by the number of distinct passages ever opened, which for a book of thousands of nodes eventually means all of them, at roughly 10 KB of document each.

  **Not a two-line fix.** Refcounting participants and dropping at zero has a real hazard:

  ```
  peer A disconnects  -> count hits 0 -> entry removed from the map
  peer B, meanwhile, is mid-hydrate and already holds the Arc
  peer C joins        -> map is empty -> hydrates a SECOND copy from the store
  ```

  Two live copies of one passage now exist, and two authors are silently invisible to each other until both persist and someone rehydrates. Worse than the leak. Two related traps: persistence happens *after* relay, so eviction must flush or it discards a relayed-but-unpersisted update; and `Arc::strong_count` looks tempting but is not a synchronisation point.

  **Intended shape:** count participants inside the map's write lock (so `hydrate` cannot interleave with a decrement-to-zero), and have a background sweeper remove entries idle for a grace period (~30 s) — the grace period is what turns the race above into a non-event, since a rejoin inside the window finds the existing entry. A guard returned by `hydrate` that decrements on `Drop` is the more idiomatic way to count (impossible to forget, survives a panic in `stay()`), but it cannot replace the sweeper: `Drop` is not async and eviction needs to await a final persist. So `Drop` marks, the sweeper collects and compacts.

  **The permanent version of the same problem:** across two server instances both can hydrate the same passage, and no amount of eviction or grace period helps. Same failure the snapshot-vs-log `absorb` analysis turned up. Single-instance eviction is tidy-up; multi-instance needs sticky routing by passage id or a shared coordination layer.
- [ ] **No backpressure on the broadcast channel.** `BACKLOG` is 256 frames; a slow peer that fills it gets a lagged receiver and silently misses frames. Its next sync recovers the prose, but awareness is lost outright.
- [ ] **Auth on the socket.** `/sync/{passage}` currently accepts anyone who names a passage that exists. Same shape as the tenancy gap in the REST layer — ids are not capabilities.
- [ ] **Generated assets must be excluded from the Trunk watch list.** The JS bundle is written into the crate by a `pre_build` hook, which retriggers the watcher; without `[watch] ignore` it rebuilds forever (190 rebuilds before it was spotted). **This already recurred once** — Playwright writing `test-results/` into `clients/web` caused 257 spurious rebuilds and a one-in-five flake, since the page reloaded mid-test. Fixed by pointing `outputDir` at `target/` and adding `e2e`/`node_modules`/the JS manifests to `[watch] ignore`. It will recur again the moment the real client gains a bundling step.

## Carried inside Phase 3

- [x] ~~**Move the `"prose"` fragment name into `contract`.**~~ Resolved differently than planned. One shared constant would have meant `core -> contract`, and keeping `core` a leaf won — so both crates define `FRAGMENT` and `features/passages/tests/src/shared_kernel.rs` asserts they match. See [the reasoning](./ARCHITECTURE.md#feature-anatomy--the-onion).

## Revisit later

- [ ] **A UI component library (thaw).** Considered and deferred, not rejected. Blocked today on versions: `thaw 0.5.0-beta` wants `leptos ^0.8.0` and we are on `0.9.0-beta`. Two reasons to still want a specific justification once that clears, rather than adopting by default: a component library is *all* components, so calling it from builder syntax means `Button(ButtonProps::builder()…)` everywhere — it largely un-decides [the builder choice](./ARCHITECTURE.md#client-conventions); and thaw is Fluent-flavoured, which fights the deliberately bookish palette (Georgia serif, warm paper) that is part of what Weaveling *is*. It also would not help with the hard part, since the prose editor is ProseMirror through interop. **Revisit when** the UI grows tables, date pickers or complex forms — the timeline and codex are where a library starts genuinely paying. The one real gap it would close today (our delete dialog has no focus trap) is answered more cheaply by native `<dialog>` + `showModal()`.

## Deferred until we actually want to deploy

Decided, not forgotten. None of this protects or enables anything today — there is nothing worth shipping yet, so this is work in service of a deployment that does not exist. Pick it up when that changes.

- [ ] **CI** — `cargo fmt --check`, `clippy`, `test`. Two gotchas found the hard way: the client needs its **own** clippy invocation (`--workspace` does not build for `wasm32`, so client lints are silently skipped), and once a Postgres backend exists CI must run **`--all-features`** or feature-gated code is never type-checked.
- [ ] **`ServeDir`** — `services/api` mounts only `/api`, so outside `trunk serve` nothing serves the app. The single-origin design the client depends on (relative `/api/…`, hence no CORS, no build-time host config, no mixed-content trap) is currently true only because Trunk proxies. Needs a `tower_http::services::ServeDir` fallback over `clients/web/dist` with an index fallback. The `fs` feature is already enabled on `tower-http`.
- [ ] **Logging + config** — `TraceLayer` emits at DEBUG while `tracing_subscriber` defaults to INFO, so there is no request log at all; needs a `RUST_LOG` default like `info,tower_http=debug`. And the bind address is hardcoded to `127.0.0.1:3000`; it should come from the environment.

## Candidate first steps (superseded by ROADMAP.md, kept for context)

1. [ ] **Domain model** — sketch the core aggregates & events: Project, Node, structural events (create / move / split / reorder), the codex entity shape, threads, time relations (before/after + buckets). This is the heart of the ES design.
2. [ ] **Scaffold the repo** — Cargo workspace, module skeleton for the modular monolith, pick the full-stack Rust framework (Leptos / Dioxus / Yew), stand up a "hello weave" client + server.
3. [ ] **Spike the risky bits** — prove the scary assumptions early:
   - [x] yrs ↔ Yjs collaborative editing round-trip (the highest-risk assumption in the whole plan).
   - [ ] embed Typst → compile a `.typ` to PDF.
4. [ ] **Keep designing** — exact event catalog, node text split-and-move mechanics, how threads are represented.

Suggested order was **#1 → #3**: nail the domain vocabulary while fresh, then de-risk the CRDT round-trip.

## Open design threads (not yet resolved)

- [ ] When a node with text is split: exact UX/data for moving ranges into children vs. leaving on parent. Two moves are now available, since [passages carry their own ids](./ARCHITECTURE.md#the-passages-feature): **re-link** the whole passage to the new node (history preserved, trivial) or **move a range** of text between two passages (history preserved only if we can transplant the CRDT items, which Yjs does not offer directly — a range move is likely a delete-plus-insert and loses provenance). Decide what an author actually expects to survive a split.
- [ ] **Multiple passages per node** — not needed yet, but the shape is known: a `role` or `variant` field on `PassageAttached`, added by event upcasting. Worth designing only once there is a second thing to put on a node (synopsis, author's notes, alternative drafts).
- [ ] "Present in a scene" vs. "merely mentioned" — do we model both relation types?
- [ ] Time model details: representing parallel nodes + nested time buckets (year ⊃ month ⊃ day).
- [x] ~~Frontend framework choice within full-stack Rust~~ — Leptos, and the editor spike confirmed it can host ProseMirror without the interop dominating.
- [ ] Event versioning/upcasting strategy.
- [ ] Auth / accounts / project membership / permissions (deferred, but looming for multi-author).
- [ ] **How to test the web client.** It has no automated tests — `Workspace`, the `ApiError` classification and `human_time` are verified only by clicking through a browser. Deliberately postponed while the client is this small. The obstacle when we do tackle it: `Workspace` calls `api` directly and `api` is gloo-net, which is wasm-only, so native unit tests would need the API behind a port (the `ProjectStore` move, applied client-side). The alternative is `wasm-bindgen-test` against headless Chrome, which tests the real thing but needs chromedriver. Decide once the frontend has enough complexity to justify one.

## Parked (decided — don't re-litigate)

- Rust, modular monolith, PostgreSQL, no broker yet (option kept open).
- Two-speed model: ES/CQRS for structure, CRDT (yrs/Yjs) for prose. **Projects stay plain CRUD** — a title carries no history worth sourcing; ES debuts on the structure tree.
- Local-first-capable client.
- Full-stack Rust first; Angular as fallback.
- Export: Typst → PDF, HTML → EPUB.
