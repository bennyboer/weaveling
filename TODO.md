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

- [x] Milestone 6 — the walking skeleton: `features/passages` end to end, two browser tabs converging on one passage through our own sync server.

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
- [ ] **Stale wasm-bindgen snippets contaminate the client bundle.** `target/wasm-bindgen/{debug,release}/snippets/` is shared by every workspace member that builds for wasm, and Trunk copies the *whole* directory into `dist`, then emits a `modulepreload` for each entry it finds. A build of `spikes/editor` left a 308 KB `editor.bundle.js` there on 16 Aug and every client release since preloaded it — 93 KB gzipped of dead code on first paint, invisible in the page source and untouched by `trunk clean`, which only cleans `dist`. Cleared by deleting `target/wasm-bindgen`, but it returns the moment the spike is built again. Neither permanent fix is free: drop `spikes/editor` from the workspace members so it emits into its own target dir (and loses `cargo test --workspace` coverage), or give the spikes a separate `CARGO_TARGET_DIR`.
- [ ] **Release minification is off, deliberately.** `[build] minify = "on_release"` was measured: CSS 4872 → 3953 raw (1558 → 1464 gzipped) and `index.html` 1500 → 1357, so about 240 bytes gzipped all told — while Trunk's JS minifier fails on the wasm-bindgen glue and prints `WARN Failed to minify JS: RequiredTokenNotFound` on every release build. Not worth a standing warning that could mask a real one. Revisit when the stylesheet is large enough for the trade to flip.

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

- [ ] When a piece with text is split: exact UX/data for moving ranges into children vs. leaving on the parent. Two moves are available, since [passages carry their own ids](./ARCHITECTURE.md#the-passages-feature): **re-link** the whole passage to a new piece (history preserved, trivial) or **move a range** of text between two passages (history preserved only if the CRDT items can be transplanted, which Yjs does not offer directly — a range move is likely delete-plus-insert and loses provenance). Decide what an author actually expects to survive a split.
- [ ] **Multiple passages per piece** — considered during M7 and deliberately deferred, not forgotten. **Deferring is safe here**, unlike the dropped `from` field: the upcast is `PassageAttached { passage }` -> `{ passage, role: Body }`, which is correct for *every* attachment that can exist before the feature lands, because until there is a synopsis every passage is the body. Lossless, one-line patch — exactly what the patcher exists for. The bill for waiting is two event version bumps (`PassageAttached` and `Snapshotted`) with a trivial patch each; the aggregate's internal `Option<PassageLink>` -> map costs nothing, since state is rebuilt from events and never stored in its own right.

  **The multiplicity is not the feature — the axis is.** A bare `Vec<PassageLink>` buys nothing usable, because the client has to know *which* passage to open in the editor, so the discriminator is load-bearing on day one rather than metadata added later. And the axis is the thing we cannot guess well: **role** (what this passage *is* to the piece — body, synopsis, author's notes; a closed set, presumably one of each) is a different question from **variant** (alternative drafts of the same role). Model `role` and later want variants, and it reshapes twice — the same failure that sank deriving a passage id from its piece id.

  **Pick it up when** a second thing genuinely wants hanging on a piece. If the answer is *role, closed set, at most one passage per role*, the shape is settled and the guess is no longer a guess:

  ```rust
  pub enum PassageRole { Body, Synopsis, Notes }

  PieceCommand::AttachPassage { passage: PassageLink, role: PassageRole }
  PieceEvent::PassageAttached { passage: PassageLink, role: PassageRole }

  struct Piece { passages: BTreeMap<PassageRole, PassageLink>, .. }
  ```

  `AlreadyHoldsPassage` becomes role-scoped, `passage()` becomes `passage(role)` with a `body()` shorthand. What to avoid is arriving there *implicitly* — a `Vec` that quietly decides the axis the first time someone needs a synopsis and picks whatever is easiest.
- [ ] "Present in a scene" vs. "merely mentioned" — do we model both relation types? Belongs to the `cast` view.
- [ ] Time model details: representing parallel pieces + nested time buckets (year ⊃ month ⊃ day). Belongs to the `timeline` view.
- [x] ~~Frontend framework choice within full-stack Rust~~ — Leptos, and the editor spike confirmed it can host ProseMirror without the interop dominating.
- [x] ~~Event versioning/upcasting strategy.~~ Settled: **every event carries its own version**, and upcasting is pure `from -> to` patch functions applied on read, so stored events are never rewritten. Taken from the earlier implementation, which had the design but never exercised it — every real event there sat at version zero. M7 must ship one real patch so ours does not go the same way.
- [ ] **Undo/redo — deferred, not designed.** Decided to leave it off the roadmap entirely for now; event sourcing keeps the option free, so there is no cost to waiting. The question to answer *when* it is picked up: per-aggregate (undo the last thing that happened to this piece) or per-author (undo the last thing *I* did, wherever it happened)? The second is what Ctrl+Z means to an author and needs the agent slot plus a cross-aggregate ordering — meaningfully more work than the first. Events already carry `from`/`to`, so whichever is chosen, the inversion is local.
- [x] ~~**Must a piece have a title?**~~ Settled: normally yes, but the empty string is a legal value — a piece captured on the board starts at `""` and is typed into. `PieceTitle` matches `ProjectName` in every rule except that it permits empty. Untitled is `""` and never `Option<PieceTitle>`, and "Untitled" is something a view draws, never something stored. See [Pieces and views](./ARCHITECTURE.md#pieces-and-views--the-non-linear-model).
- [ ] **A live channel for the board.** Awareness is bound to `/sync/{passage}` today, and a board is not a passage. M8 needs a second live surface that carries committed events plus in-flight drags. The earlier implementation had precedent worth copying: a WebSocket pushing event messages, with per-event handlers patching a local store on the client.
- [ ] Auth / accounts / project membership / permissions (deferred, but looming for multi-author).
- [x] ~~**How to test the web client.**~~ Settled: Playwright end-to-end against `trunk serve` plus the real API, `clients/web/e2e`, 14 tests. The obstacle recorded here — that `Workspace` called gloo-net directly, so native unit tests would need the API behind a port — turned out to argue *for* the end-to-end choice rather than against it. See [Client conventions](./ARCHITECTURE.md#client-conventions).

## Parked (decided — don't re-litigate)

- Rust, modular monolith. **No broker** — `libraries/messaging` owns the port, the envelope and an in-process dispatcher; a broker is an adapter for when a second deployable exists. See [Messaging](./ARCHITECTURE.md#messaging--the-seam-now-the-transport-later).
- ~~PostgreSQL~~ **reopened.** MongoDB is back under consideration; the decision moved to M11, judged by the conformance suites. Everything above a port is unaffected either way, which is the whole reason the choice can wait.
- Two-speed model: ES/CQRS for structure, CRDT (yrs/Yjs) for prose. **Projects stay plain CRUD** — a title carries no history worth sourcing; ES debuts on `pieces`.
- **The tree is a view, not the model.** Pieces are a pool; `board`, `outline`, `timeline`, `threads` and `cast` each own their own arrangement of them. Position is never a property of a piece. See [Pieces and views](./ARCHITECTURE.md#pieces-and-views--the-non-linear-model).
- Local-first-capable client.
- Full-stack Rust first; Angular as fallback.
- Export: Typst → PDF, HTML → EPUB.
