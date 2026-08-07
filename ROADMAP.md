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

*Done. Six crates, `cargo build` green, `/api/health` returns 200, client type-checks for `wasm32`. See the README for how to run it.*

## Milestone 1 — The domain: `Project`

**Goal:** a `Project` type you're happy with, plus tests.

**Build:**
- `Project` — id, name, timestamps. Consider a newtype `ProjectId` over `Uuid` rather than a bare `Uuid`.
- Validation: what makes a name valid? Empty names rejected, trimmed, length bounds.
- Unit tests for the validation rules.

**Rust you'll meet:** structs and `impl` blocks, derive macros (`Debug`, `Clone`, `PartialEq`), the **newtype pattern**, `Option` vs. `Result`, constructing fallible values, `#[cfg(test)] mod tests`.

**Done when:** `cargo test` passes and creating an invalid project is impossible by construction.

## Milestone 2 — The store trait + in-memory impl

**Goal:** the abstraction that makes Postgres a later, cheap decision. The heart of Phase 1.

**Build:**
- A `ProjectStore` trait: create, get, list, rename (or a general update), delete.
- Its error type — a `StoreError` enum (`NotFound`, `Conflict`, `Backend`), deliberately *not* leaking any storage technology into the signature.
- An `InMemoryProjectStore` implementing it, holding a `HashMap<ProjectId, Project>`.
- Tests written **against the trait**, so the same suite can later validate the Postgres impl for free. The conformance suite lives in `core` behind a cargo feature (`testkit`); the store adapter picks it up as a dev-dependency.

**Crate placement:** the `ProjectStore` trait belongs in `core` — a port is declared by the domain that needs it. The `HashMap` impl goes in `adapters/store`. Nothing above the trait knows which impl exists; the service picks.

**Rust you'll meet:** traits and trait bounds, `async` in traits, **shared mutable state** (`Arc` + `RwLock`/`Mutex`, interior mutability), static vs. dynamic dispatch (generics vs. `dyn Trait`), error modelling with `thiserror`, `?` and `From` conversions.

**This is where the design lives.** Expect to spend real time here and expect me to push back on the trait shape — if it's leaky, everything downstream inherits the leak.

**Done when:** the trait test suite passes against the in-memory impl, and nothing above the trait knows what's behind it.

## Milestone 3 — HTTP API

**Goal:** projects are fully manageable over REST.

**Build:**
- `GET /projects`, `POST /projects`, `GET /projects/{id}`, `PATCH /projects/{id}`, `DELETE /projects/{id}`.
- Request/response DTOs, kept separate from the domain types.
- Store injected as shared application state.
- Errors mapped to sensible status codes (404, 400, 409).

**Rust you'll meet:** axum routing, handlers, extractors (`Path`, `Json`, `State`), **serde** derive and attributes, implementing `IntoResponse` for your error type, the async/await model and why the store needs `Send + Sync`.

**Done when:** the full lifecycle works via `curl`, and errors return the right codes rather than 500s.

## Milestone 4 — Frontend

**Goal:** a browser UI listing projects, with create / rename / delete.

**Decision needed before starting:** which full-stack Rust framework. **Recommendation: Leptos** — the largest ecosystem of the three, signal-based reactivity that will feel familiar, and the best documentation for someone getting back into Rust. Dioxus and Yew stay open; nothing in M0–M3 depends on the choice.

**Build:**
- Project list view, plus create / rename / delete.
- Talk to the API over HTTP; handle loading and error states honestly.
- Sort out CORS or same-origin serving.

**Rust you'll meet:** the WASM target and toolchain, components and signals, async data fetching from the browser, and the reality of debugging Rust in a browser.

**Done when:** the whole loop works in a browser with no `curl` involved.

**Escape hatch:** if the editor/typography story later proves too painful in Rust/WASM, the frontend switches to Angular against the same API. Phase 1 is small enough that finding this out here is cheap — that's part of the point.

## Milestone 5 — Tidy up

Small, worth doing before moving on: structured logging (`tracing`), config via environment, a README section on running it locally, CI that runs `cargo fmt --check`, `cargo clippy` and `cargo test`.

---

## After Phase 1 (sketch only)

Not planned in detail — we'll sequence these once Phase 1 lands and we know how the code actually feels.

- **Nodes and the structure tree** — the real domain. Where event sourcing starts to earn its place.
- **Prose + the yrs ↔ Yjs round-trip** — the highest-risk assumption in the whole design. De-risk early once there's something to hang it on.
- **Postgres** — write the second `ProjectStore` impl and prove the abstraction was worth it.
- Then, in some order: codex, timeline, threads, export, accounts.

## Explicitly not in Phase 1

Postgres · event sourcing · CRDTs · WebSockets · auth · the codex · the timeline · threads · export · offline support.
