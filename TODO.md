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

## Deferred until we actually want to deploy

Decided, not forgotten. None of this protects or enables anything today — there is nothing worth shipping yet, so this is work in service of a deployment that does not exist. Pick it up when that changes.

- [ ] **CI** — `cargo fmt --check`, `clippy`, `test`. Two gotchas found the hard way: the client needs its **own** clippy invocation (`--workspace` does not build for `wasm32`, so client lints are silently skipped), and once a Postgres backend exists CI must run **`--all-features`** or feature-gated code is never type-checked.
- [ ] **`ServeDir`** — `services/api` mounts only `/api`, so outside `trunk serve` nothing serves the app. The single-origin design the client depends on (relative `/api/…`, hence no CORS, no build-time host config, no mixed-content trap) is currently true only because Trunk proxies. Needs a `tower_http::services::ServeDir` fallback over `clients/web/dist` with an index fallback. The `fs` feature is already enabled on `tower-http`.
- [ ] **Logging + config** — `TraceLayer` emits at DEBUG while `tracing_subscriber` defaults to INFO, so there is no request log at all; needs a `RUST_LOG` default like `info,tower_http=debug`. And the bind address is hardcoded to `127.0.0.1:3000`; it should come from the environment.

## Candidate first steps (superseded by ROADMAP.md, kept for context)

1. [ ] **Domain model** — sketch the core aggregates & events: Project, Node, structural events (create / move / split / reorder), the codex entity shape, threads, time relations (before/after + buckets). This is the heart of the ES design.
2. [ ] **Scaffold the repo** — Cargo workspace, module skeleton for the modular monolith, pick the full-stack Rust framework (Leptos / Dioxus / Yew), stand up a "hello weave" client + server.
3. [ ] **Spike the risky bits** — prove the scary assumptions early:
   - [ ] yrs ↔ Yjs collaborative editing round-trip (the highest-risk assumption in the whole plan).
   - [ ] embed Typst → compile a `.typ` to PDF.
4. [ ] **Keep designing** — exact event catalogue, node text split-and-move mechanics, how threads are represented.

Suggested order was **#1 → #3**: nail the domain vocabulary while fresh, then de-risk the CRDT round-trip.

## Open design threads (not yet resolved)

- [ ] When a node with text is split: exact UX/data for moving ranges into children vs. leaving on parent.
- [ ] "Present in a scene" vs. "merely mentioned" — do we model both relation types?
- [ ] Time model details: representing parallel nodes + nested time buckets (year ⊃ month ⊃ day).
- [ ] Frontend framework choice within full-stack Rust (Leptos vs Dioxus vs Yew).
- [ ] Event versioning/upcasting strategy.
- [ ] Auth / accounts / project membership / permissions (deferred, but looming for multi-author).
- [ ] **How to test the web client.** It has no automated tests — `Workspace`, the `ApiError` classification and `human_time` are verified only by clicking through a browser. Deliberately postponed while the client is this small. The obstacle when we do tackle it: `Workspace` calls `api` directly and `api` is gloo-net, which is wasm-only, so native unit tests would need the API behind a port (the `ProjectStore` move, applied client-side). The alternative is `wasm-bindgen-test` against headless Chrome, which tests the real thing but needs chromedriver. Decide once the frontend has enough complexity to justify one.

## Parked (decided — don't re-litigate)

- Rust, modular monolith, PostgreSQL, no broker yet (option kept open).
- Two-speed model: ES/CQRS for structure, CRDT (yrs/Yjs) for prose. **Projects stay plain CRUD** — a title carries no history worth sourcing; ES debuts on the structure tree.
- Local-first-capable client.
- Full-stack Rust first; Angular as fallback.
- Export: Typst → PDF, HTML → EPUB.
