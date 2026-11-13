# Weaveling — Architecture

This document is the *how*. For the *what* and the *why*, see [README.md](./README.md).

This captures the architectural decisions we've settled on so they aren't lost. Some are built, some are still design, and some were **proven by a spike** — where a claim says *proven*, a test or a verified browser session stands behind it. Where a decision is provisional, it says so.

## Shape

Weaveling is a **client–server application**.

- **Server:** a **modular monolith** written in **Rust**.
- **Client:** a browser. "Write from anywhere with just a browser" is a core goal.
- **Database:** **PostgreSQL**.

The server is a monolith *for now*, but organized into clean modules so that seams can later become process/service boundaries without a rewrite.

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

Today's libraries are `clock`, `ids`, `eventsourcing`, `messaging`, `eventpublishing`, `serving` and `words`. `words` is different from the others. It has no port. It only holds the rule for what counts as a word. The scenes `core` and the WASM client must both apply this rule in exactly the same way. `words` depends on nothing, so the client can use it.

`eventpublishing` and `serving` are the only libraries that depend on other libraries. The reason is the same for both: each one is a **join** between two other libraries.

- `eventpublishing` turns aggregate events into published messages.
- `serving` turns event-sourcing outcomes into HTTP. It turns `If-Match` into a `Version`, a `Version` into an `ETag`, and a `ServiceError` into a status code.

**`serving::refusal` maps a domain refusal to 409, and that is a default a feature may override.** 409 is right when the caller could resolve the conflict and then retry. An example is pinning an idea that is already pinned. 409 is wrong when the refusal means that the addressed thing simply does *not exist*. An example: `PATCH /boards/{board}/ideas/{idea}` for an idea that was never pinned is refused as `NotPinned`. The correct answer here is **404**, because the URL names a placement that does not exist. So the feature's adapter checks for that one refusal first, and only then hands the rest to the library. The rule is: the library knows HTTP, and the feature knows which of its own refusals mean *absent* rather than *conflicting*.

**Why many crates:** in Rust a crate boundary *is* an enforced dependency rule. "Persistence must not leak into business logic" stops being a code-review convention and becomes a compile error, because the crate simply doesn't list the dependency. Crates also give parallel compilation and caching.

**Aim for a wide, shallow graph.** Crate count alone doesn't shorten builds — a crate is the unit of both parallelism *and* recompilation, so a deep chain rebuilds serially all the way up. Libraries should be stable leaves; depth is the enemy.

### Feature anatomy — the onion

Each feature is a slice of the domain (project management, ideas, scenes, board, timeline, …) and is internally an **onion**: a pure core, wrapped in a ring of adapters.

```
features/
└── projects/
    ├── contract/          weaveling-projects-contract   → —      (shared kernel, WASM-safe)
    ├── core/              weaveling-projects-core       → —      (domain, ports, ProjectsCore)
    ├── adapters/
    │   ├── rest/          weaveling-projects-rest       → core, contract   (inbound)
    │   └── store/         weaveling-projects-store      → core             (outbound)
    └── tests/             weaveling-projects-tests      → the whole feature (test-only leaf)
```

```
     contract         core
         ↑           ↑    ↑
         └── rest ───┘    └── store
               ↑              ↑
               └─── service ──┘
```

- **`core`** — domain types, business logic, the **ports** (traits) it needs, and the application facade (`ProjectsCore`). Plain Rust, no frameworks, no magic. **Depends on no other crate in its own feature** — in particular not on `contract`, so the wire format can never leak inward. Cross-cutting library ports are the one exception (see [cross-cutting ports](#dependency-rules)); `libraries/clock` is why `core` is a near-leaf rather than a strict one.
- **`adapters/*`** — **one crate per adapter**, named for the foreign system it talks to. All arrows point inward at `core`.
  - *Outbound (driven)* adapters implement a port declared by core. Named after the port: `ProjectStore` → `store`. A port with several **interchangeable backends** stays **one** crate — `memory` and later `postgres` are modules inside `store`, not sibling crates. They are backends of one adapter, not two adapters. Heavy optional backends go behind a cargo feature (`postgres = ["dep:sqlx"]` with `sqlx` marked `optional`), so a build that never enables it never compiles it. Two consequences: the swap is one feature flag rather than one dependency line, and **CI must run `--all-features`**, because feature-gated code is not type-checked otherwise.
  - *Inbound (driving)* adapters call core's public API. Named after the transport: `rest`, later `graphql`, `cli`, `messaging`. No inbound port trait — `ProjectsCore` is already the interface.
- **`contract`** — the wire types. It is deliberately **not** part of the onion. It is a shared kernel between two *processes*. It exists as its own crate for a hard technical reason: the WASM client cannot depend on `rest`, because `axum` does not compile to WASM. The rule for this crate is **`serde` and nothing else**. Values travel in their simple wire form, so ids and timestamps are strings. The rich domain types (`ProjectId`, time types) stay in `core`, and the adapter converts between the two. This keeps the crate WASM-safe without effort, and it means the domain vocabulary has exactly one owner. Note that ids coming *in* arrive through the URL path, and `rest` parses them with its own extractor. So `contract` is almost entirely about the shape of outgoing data.

**A constant both processes must agree on lives in *both*, pinned by a test.** The fragment name a scene's prose lives under (`"prose"`) is needed by `core` to project plain text and by the client to write into the same document — and `core` must not reach into `contract` to get it. So each defines its own, and the feature's tests crate, which can see both, asserts they match:

```rust
assert_eq!(scenes_core::FRAGMENT, scenes_contract::FRAGMENT);
```

A duplicated literal justified by a test is normally a smell. It wins here because the alternative is worse: one `core → contract` arrow "just for a constant" is the crack DTOs leak through later, and the invariant is only worth having while it is absolute. The failure this test prevents is a silent one — mismatched fragment names mean prose that vanishes with no error anywhere.

**One way into a feature: its core.** Every feature's `core` exports one struct named after the feature: `ProjectsCore`, `IdeasCore`, `ScenesCore`. Its methods are everything the feature does:

- what a user does (reading, writing, following),
- what the feature does by itself when it receives a message (cataloguing, sweeping, removing references to things deleted in another feature),
- its chores.

Every adapter goes through this struct, the inbound adapters and the chores alike. Nothing else in the core can be called from outside.

That makes the core the one place where permissions are checked. The core asks its `Authorizer`, or checks whether the agent is an operator. Only then does it pass the call on to a service. A service carries out commands and records who acted. A service checks no permissions, and it is not exported. A bigger feature may keep several services behind one core.

When the system does work on its own, because of a message, that work is done and recorded as `Agent::System`.

Each core has one error enum, for example `ProjectsError`. A write returns what it wrote, so no adapter has to read it back on its own.

Live updates stay out of the core. The core only answers whether an agent may follow something. `libraries/live` holds the socket.

*Decided on 2026-10-25 by the user. The reasons: a reader sees in the struct everything the feature is about, and a permission check that is missing from one adapter cannot hide in another.*

**Each feature knows itself who may do what.** While running, no feature asks another feature whether someone may read or change a project, or whether someone operates the instance. Instead, each feature keeps its own copy of the answer in its own database, using [`libraries/access`](./libraries/access/README.md). This copy holds:

- the roles,
- the projects that are disbanded or in the trash,
- the operators.

The feature learns these from the events of the members, projects and accounts features. It receives them with listeners of its own. Its core then asks `KeptAccess`, which is the `Authorizer` built on this copy. When accounts are turned off, the wiring gives the core `AllowingEverything` instead (`Context::with_accounts`).

The members feature is where roles are decided. It keeps its own `MembersAuthorizer`, built on its own catalog.

A change reaches the other features some milliseconds after it is made. During that short time, someone who just joined may be refused, and someone who just left may still be let in.

**The library knows no feature's contract.** The contracts that publish the events provide the listeners as macros:

- `members_contract::access_roles_listener!`
- `projects_contract::access_trash_listener!`
- `accounts_contract::access_operators_listener!`

Each macro is used as one line in a feature's messaging crate. The listener passes what it receives to the feature's core through the traits `HearsAccess` and `HearsOperators`. The library declares these traits, and each core implements them.

*Decided on 2026-11-13 by the user. The features become independent of members and accounts while running. The cost is the delay described above, and the user accepted it. The rule that libraries know no feature stays strict. At first the listeners were written the same way in each feature. On the same day, at the user's request, they became macros in the contracts.*

There is deliberately **no `boundary` crate**. Once every adapter has its own crate, "boundary" names whatever is left over — which is nothing. Mapping belongs to the adapter that owns the DTOs, the facade lives in core, and wiring is the service's job.

**When *not* to split an adapter out:** the test is *different dependencies, or a different swap story?* REST vs. store: yes to both — split. REST vs. a hypothetical GraphQL adapter over identical DTOs: no to both — one crate. Two backends of the same port are never split: they answer "no" to the swap question by definition, since they are alternatives to each other rather than things that coexist. Split on dependency weight and swappability, not on conceptual tidiness.

**Feature-level tests live in their own crate.** `features/<name>/tests` is **test-only**: an empty library plus `#[cfg(test)]` modules, depending on every crate in the feature and depended on by nothing. It exists because cross-adapter tests have nowhere else legal to live — putting them in `adapters/rest` would need a `rest → store` dev-dependency (the forbidden sibling arrow), and putting them in `core` would need `core → adapters/store`, inverting the onion. A leaf that depends on the whole feature keeps every arrow pointing inward.

The payoff is that feature tests use the **real** adapters. A hand-written fake store is a second, unverified implementation of the port — it cannot run against the conformance suite, so it is free to drift from the contract and let tests pass against behavior no real backend exhibits. Using `InMemoryProjectStore` removes that hazard entirely. It is deliberately **not** a wiring crate: no composition happens here, because that is still the service's job. Give it a test-only crate's dependency list — everything under `[dev-dependencies]`, nothing under `[dependencies]`.

There are four test scopes, and keeping them apart matters:

| Scope | Asks | Lives in |
|---|---|---|
| **Unit** | is this type's logic right? (`ProjectName` validation, id ordering) | the crate that owns the code |
| **Port conformance** | does every backend honor what the port promises? | `adapters/<port>/src/suite.rs` |
| **Feature behavior** | does the feature behave correctly through its facade and router, with real adapters? | `features/<name>/tests` |
| **App wiring** | is the feature mounted at the right path with the right middleware? | `services/api/tests` — smoke tests only |
| **Client behavior** | does the browser actually do the right thing? | `clients/web/e2e` — Playwright, real browser, real stack |

Feature tests are written **from a business perspective**: they assert observable outcomes, never collaborations. "The store was not called" is a technical detail invisible to any client, and asserting it couples the test to the current call order. Spy on a collaborator only when the interaction *is* the requirement — don't charge a card twice, don't send two emails — which a read from our own store never is.

**Port conformance is tested once per port, inside the adapter crate.** Because a port's backends share one crate, the suite that defines what the port *means* — `create` conflicts on a duplicate id, `update` is `NotFound` on a missing one, `list` returns creation order — lives in a plain `#[cfg(test)]` module there (`store/src/suite.rs`), taking `&impl ProjectStore`. Every backend runs the same cases; backend-specific behavior (transaction rollback, connection failures mapping to `StoreError::Backend`) goes in that backend's own tests. `core` stays free of test scaffolding.

**Adapters are siblings, never friends.** `rest` must not depend on `store`. Choosing the concrete persistence implementation is the **service's** job as composition root — that is what keeps the in-memory → PostgreSQL swap down to one line in one manifest.

### Dependency rules

| Layer | May depend on | Never |
|---|---|---|
| `clients` | contract crates | features, services |
| `services` | features, libraries | clients |
| `features` | libraries, own contract (adapters only — never `core`) | **other features** |
| `libraries` | libraries (shallowly) | features, services |

The **feature → feature** ban is the load-bearing one. When two features want the same thing there are exactly two legal moves: the *service* composes them, or the shared concept sinks into a `libraries/` crate. (First real case: `ideas` and `boards`, resolved by neither move — the client joins them, which is legal because `clients` may depend on contract crates. See [the board join](#the-board-renders-through-a-frontend-join).)

**Each feature owns its own database, and that is what makes the ban physical.** The compiler enforces the crate table above. But the table says nothing about the storage underneath. A foreign key that crosses a feature boundary would couple two features again, and no Rust rule would catch it. So every feature gets its **own PostgreSQL database** on the shared cluster. Not its own schema, and not a shared database. The reason: PostgreSQL refuses a cross-database reference outright (`cross-database references are not implemented`). So a join or a foreign key between features is not just forbidden — it cannot be written at all. This is also why references to another feature use opaque link types such as `IdeaLink` and `ProjectLink`. The feature that makes the reference declares them. They are never imported from the feature being referred to.

This is not an extra rule. It is the same reasoning as the crate boundary: two things that must not know about each other are kept where they *cannot* know about each other. The cost is one connection pool and one set of migrations per feature. In return, a feature can be moved into its own service with `pg_dump`. And a whole class of schema decisions that quietly tie two features together becomes impossible.

**The consequence for the outbox.** PostgreSQL has no transactions across databases. The [transactional outbox](#messaging--the-seam-now-the-transport-later) only works because the event rows and the outbox rows commit in the same transaction. So each event-sourced feature's database holds **its own `events` and its own `outbox`**. A feature that is moved out takes its outbox with it. No feature may write to another feature's outbox. It follows that there is deliberately **no global ordering** across features. One sequence across everything would be exactly the coupling this whole setup is built to prevent. So nothing may be built that depends on such an ordering.

**In tests it is a schema per feature rather than a database per feature.** `CREATE DATABASE` copies a template directory and cannot run inside a transaction. A database per feature per test would therefore be far too slow. `libraries/test-harness` gives a test a `PostgresFixture`. The fixture gives each feature its own schema inside one throwaway database. The adapters never name the schema or the database — they use the pool they are given. The known gap: in a test, a join across schemas is *possible*, while in production it is impossible. So the wall is only real in the shipped setup.

**Cross-cutting ports live in `libraries/`, not in a feature's `core`.** Normally a port is declared by the core that needs it. That rule holds for domain-specific ports like `ProjectStore`. A port that is *domain-free* and needed by every feature goes into a library instead. `libraries/clock` is the first one. It holds the `Clock` trait plus `SystemClock` and `FixedClock`. The reason is stronger than expected reuse: copies of `Clock` in each feature would be **incompatible types**. The composition root would then have to build one clock per feature, and no test could fix the time across the whole system. This is legal under the table above (`features` → `libraries`) and is an explicit *exception* to port ownership, not a violation of it.

Note that `FixedClock` is plain `pub`, not `#[cfg(test)]` — test scaffolding in a library crate must be, since `cfg(test)` does not cross crate boundaries. That is fine here because a fixed clock is legitimately useful outside tests (replay, backfill).

### Naming and conventions

- **American English throughout** — `color`, not `colour`. Not a preference about English so much as about having one spelling per word: the moment both exist, every identifier and every grep becomes a coin flip. The libraries we sit on (`color` in y-protocols awareness, CSS `color`) are American already, so the alternative was mixing spellings inside a single line.
- **Prefix every crate.** Cargo crate names are flat and global; directories only group. `weaveling-projects-core`, not `core`.
- **Alias dependencies to stay readable.** `projects-core = { package = "weaveling-projects-core", path = "../core" }` gives `use projects_core::Project;` at the call site. Set this up from the first manifest — retrofitting means touching every import.
- **Shared versions in `[workspace.dependencies]`**, with member crates writing `serde = { workspace = true }`. One place to bump, and it prevents split-version build explosions. Glob members let new crates auto-join, but the patterns must match the crate depth exactly (`"features/*/contract"`, `"features/*/core"`, `"features/*/adapters/*"`) — a greedy `**` also matches intermediate directories that hold no `Cargo.toml`.
- **`features/` collides with Cargo features** (`[features]`, `--features`, `#[cfg(feature = "…")]`). Accepted knowingly: legibility to any programmer beat avoiding the clash. `modules/` collides with `mod` and `domains/` is more jargon, so there was no clean winner.

### Consequences in Rust

Three practical decisions that follow from the layout:

- **Ports use `Arc<dyn Trait>`, not generics.** A `<S: ProjectStore>` type parameter is infectious — it propagates into every caller and fights axum's `State` extractor. The vtable lookup is noise next to I/O.
- **Async traits need `async-trait`.** `async fn` in traits is stable but *not* dyn-compatible, so `Arc<dyn ProjectStore>` requires the boxing that `#[async_trait]` provides.
- **DTO mapping is free functions, not `From` impls.** The orphan rule forbids `impl From<Project> for ProjectDTO` inside `rest`, since neither type is local to it. Plain `fn to_dto(p: &Project) -> ProjectDTO` has no such restriction and keeps the arrows correct. Making `core` depend on `contract` to get the impls would leak the wire format into the domain.

## Identifiers

**UUID v7** ([RFC 9562](https://www.rfc-editor.org/rfc/rfc9562), 2024) everywhere — not v4. A 48-bit Unix-millisecond prefix followed by 74 random bits, so ids are lexicographically sortable and sort order equals creation order.

- **Why:** monotonic keys append at the right edge of a B-tree instead of scattering inserts across it — fewer page splits, less dirtied cache, less WAL. Postgres uses heap tables rather than a clustered index, so the effect is milder than the MySQL horror stories, but the primary-key index still pays. This is irrelevant for projects (dozens per user) and decisive for **ideas and scenes** (thousands per book) and the **event log** (append-only, the most insert-heavy table in the whole design). Setting one convention now beats choosing per entity later. Bonus: `ORDER BY id` *is* creation order, so cursor pagination needs no extra column.
- **Cost:** an id leaks its creation time to millisecond precision, and entropy drops from 122 random bits to 74 (still far past any collision concern). Acceptable for a login-gated tool; revisit if ids ever land in public share URLs.
- **The timestamp is injected, never read inside the domain.** Use `Uuid::new_v7(Timestamp::from_unix(…))` fed by the same `now` the aggregate receives — not `Uuid::now_v7()`, which would smuggle a clock read into `core`. This keeps the domain pure, keeps tests deterministic, and makes an id's embedded timestamp agree with its aggregate's `created_at` by construction rather than by luck.

**Ids are prefixed on the wire and in logs: `project_019a4f…`, `scene_019a4f…`.** The prefix is mechanical — the aggregate's type name, lowercased and singular, plus an underscore — so `thread_` and `codex_` need no decision when they arrive. Inside the server `ProjectId` and `SceneId` are distinct newtypes, so the compiler already prevents mixing them — but the moment an id becomes a string (a URL, a JSON field, a log line, a bug report) that protection is gone. The prefix restores it exactly where it was missing.

- **Where it lives: `core`, on `Display`/`FromStr`.** The layering argument says a prefix is a wire concern belonging in `to_dto`; that is overruled because the biggest benefit is *logs*, and `StoreError::NotFound(SceneId)` formats through `Display`. Put the prefix in the adapter and every internal log line still shows a bare UUID.
- **`FromStr` requires the prefix** rather than accepting either form, in HTTP routes included. Leniency would defeat the point: a bare UUID parses as *any* id type, which is the confusion being eliminated. Sending a project id to `GET /scenes/{id}` is now a **400 saying so**, not a 404 indistinguishable from "deleted". Accepting both forms was considered and rejected — it re-opens that hole for bare ids, and strict-then-lenient is a harmless relaxation while lenient-then-strict is a breaking change. The friction it would have saved (pasting a bare id from psql into curl) is answered by the error message instead, which names the prefix it wanted.
- **`as_uuid()` is unchanged**, so PostgreSQL still stores a native `uuid` — 16 bytes, indexable. The prefix is a presentation form, which does mean **the database and the wire disagree**: a hand-written query returns `019a4f…` and you must prepend `scene_` to use it against the API. Accepted for the log benefit; it will bite someone once.
- **Sortability survives.** The prefix is constant per type, so `ORDER BY id` on the string form is still creation order.
- **The prefix is for humans, not for dispatch.** Never branch on `id.starts_with(…)` — that is what the newtypes are for.

## The Two-Speed Model

The single most important architectural decision. The domain splits into two regimes with very different characteristics, and each gets the tool that fits it.

### 1. Structure — Event Sourcing + CQRS

Coarse-grained, relatively low-frequency changes where history has real value:

- capture, retitle and discard ideas
- pin, move and unpin them on a board
- place scenes in the outline, or reorder what is already there
- attach a codex entity to an idea
- add or route a thread
- set time buckets and before/after relations

The vocabulary is settled in [Ideas, scenes and views](#ideas-scenes-and-views--the-non-linear-model): ideas and scenes are the two pools, and each view owns its own arrangement of them.

This regime uses **event-sourcing + CQRS**. It buys us a full audit ("who changed what"), time-travel, a natural basis for merging structural changes between collaborators, and undo/redo whenever we want it. Note the ordering: the audit log is what we are actually building for, and **undo/redo is deliberately not on the roadmap yet** — event sourcing keeps the option free, which is different from the feature being in scope.

### 2. Prose — CRDT

The actual typing inside a scene is character-level, high-frequency, and needs **intention-preserving merge** when multiple authors edit the same paragraph at once. Event-sourcing does **not** solve this on its own, and routing keystrokes through command validation + projections would make typing laggy.

Prose therefore uses a **CRDT**:

- **`yrs`** (the Rust port of Yjs / y-crdt) on the server,
- **Yjs** on the client, bound to the editor.

CRDT updates *may* still be persisted into the durable log, but they never pass through command-validation or a broker.

The editor, the shared type and the projection are settled in [Prose — the editing stack](#prose--the-editing-stack).

### Presence is neither

Cursor position, selection, and who's-online are **ephemeral awareness** state. They are **never** event-sourced and **never** persisted — they travel over the WebSocket (Yjs awareness protocol) and are discarded.

## Prose — the editing stack

Unlike the rest of this document, these decisions are **backed by running spikes** (`spikes/crdt`, `spikes/sync`, `spikes/editor`) rather than by reasoning alone. Every claim below that says *proven* is a test or a verified browser session.

**Editor: ProseMirror.** Chosen over Quill for **decorations** — a presentation layer painted *over* the document rather than into it. Spellcheck squiggles, codex mention highlights, thread anchors and later "who wrote this" are all views, not content: they must not end up in the CRDT, in the export, or in another author's replica. Quill's Delta model has no equivalent — highlighting a word means writing a format attribute into the content. ProseMirror also brings a real schema, which is what makes "an author may put a table here" a checkable statement. Licensing: ProseMirror, `y-prosemirror` and Yjs are all MIT, so a proprietary product is fine.

**Shared type: `Y.XmlFragment`, not `Y.Text`.** A scene holds whatever the author put in it. `Y.Text` models one flat run of characters with marks and cannot express block structure at all. `y-prosemirror` maps a ProseMirror document onto an `XmlFragment` of `XmlElement`s and `XmlText`s — that is the thing we store, sync and persist.

**Scene granularity belongs to the author.** A scene is *not* defined to be a paragraph. One scene per chapter is legal, and so is one per paragraph; nothing in the system may assume either. The consequence is that **merge granularity follows scene granularity**: two authors inside one scene share a CRDT and contend, two authors in sibling scenes never touch. Coarser scenes mean more contention — an author's trade-off, not a constraint we impose.

**One CRDT document per scene.** Not one per book. A book-sized document means every reader loads the entire history and every keystroke touches one hot object. Per scene keeps updates small, allows lazy loading during an in-order walk, and makes compaction a per-scene job that can run incrementally.

**Images are references, not bytes.** An `image` node carries a `src` into blob storage. Bytes embedded in a CRDT are bytes in *every* replica forever — including the tombstoned ones nobody can see. This is the same decision as blobs under [Supporting Concerns](#supporting-concerns-noted-for-later), arrived at from the opposite direction.

**Plain text is a projection, never a second source of truth.** The server derives it by walking the fragment; it is what search indexing (Tantivy), mention detection and the export walk consume. *Proven:* `yrs` reads a fragment written by `y-prosemirror` and extracts prose byte-identical to ProseMirror's own `textBetween`. The rule that makes them agree is narrower than it looks — collect **textblocks** (`paragraph`, `heading`, `code_block`) and join with `\n`; containers (`blockquote`, `bullet_list`, `list_item`) contribute no separator of their own, and an *empty* textblock still contributes one. A paragraph holding only an image is empty prose but a real blank line. Getting this rule wrong is exactly how a projection drifts silently from the editor's own notion of the document, which is why the test asserts against ProseMirror's output rather than a hand-written string.

**Wire compatibility is proven, not assumed.** `yrs` 0.27.3 ↔ Yjs 13.6.32 exchange updates and state vectors in both directions and converge — including the case that actually matters, two clients inserting into the *same gap*, where only matching tie-break rules agree. A `yrs`-authored edit also re-parses into a schema-valid ProseMirror document (`node.check()` passes), so the server can touch prose without corrupting it.

**The `prosemirror` Rust crate is not needed.** [It exists](https://docs.rs/prosemirror/latest/prosemirror/), but `yrs` walks the `XmlFragment` directly. Adopting it would add a third representation — fragment → its JSON → its node tree — to keep in step, in exchange for schema validation and transforms the server does not currently perform. Schema validity is the client's job today. Keep it in the back pocket for the day the server has to *author* structural prose changes (an importer, a bulk find-and-replace) rather than read them.

**Compaction is mandatory, not an optimisation.** 500 rewrites of a single paragraph:

| | `Y.Text` | `Y.XmlFragment` |
|---|---|---|
| final text | 490 B | 121 B |
| append-only log | 110 582 B | 27 638 B |
| compacted log | 8 566 B | 9 444 B |
| snapshot | 5 420 B | 10 308 B |

The columns are different documents, so read down, not across: in both cases the naive append-only log is **11–13× larger** than the compacted form, and per edit the `XmlFragment` costs roughly twice what `Y.Text` does — structure is not free. An afternoon of real editing on one paragraph is enough to make storing raw updates untenable.

The distinction that matters: `Y.mergeUpdates` is **compaction, and lossless** — it collapses redundant and adjacent items but keeps every `(clientID, clock)` id, so a replica that has been offline for a month still receives a correct diff. **Truncation** — dropping old updates outright — would silently break exactly that replica and is not on the table. Tombstone GC is a separate, also-lossless lever (measured: 5 420 B with GC vs. 9 420 B without), but it is disabled while snapshots/history are in use, so we cannot rely on it.

### The client hosts the editor; Rust never touches the document model

*Proven:* a Leptos component mounts a ProseMirror editor, drives it, and tears it down cleanly. The pattern is an **uncontrolled region** — Leptos renders an empty `<div>` and never looks inside it again; ProseMirror owns that subtree. Mounting happens in an effect once the `NodeRef` resolves, and `on_cleanup` destroys the view, the awareness and the Y.Doc, so a route change leaks nothing.

**The interop boundary is bytes, not structure.** The entire JavaScript surface is a ~20-line `#[wasm_bindgen(module = "…")]` block against a real ES-module import — no globals, no string-keyed dispatch. Rust hands over `&[u8]` and receives `Vec<u8>`; the only richer thing crossing is a plain-text read. This is the load-bearing decision: **Rust never manipulates the ProseMirror model**, so the two sides can evolve independently and the editor could be swapped without touching the server. The `prosemirror` Rust crate stays unnecessary for the same reason.

One wrinkle worth remembering: wasm-bindgen implements `Send`/`Sync` for `JsValue` (absent atomics), so a JS handle can live in a normal `StoredValue`, but a Rust `Closure` cannot — callbacks need `StoredValue::new_local`.

**Cost:** ~**334 KB gzipped** for the whole collaborative editing stack — 239 KB wasm (Leptos + `yrs`), 97 KB JS bundle (ProseMirror + Yjs + `y-prosemirror` + `y-protocols`), 7 KB glue. Against ~158 KB for the projects client, that is roughly **+180 KB** to add collaborative rich text, split about evenly between the wasm and the JS. Acceptable, and lazy-loadable — a reader browsing the project list should never pay it.

### Sync speaks the y-protocols wire format, over WebSocket

*Proven:* real `y-websocket` clients synchronise through a hand-written Rust server. Two clients converge on concurrent edits, a client that joins **after** an edit is caught up, and awareness reaches the other peer.

**We implement the protocol rather than depend on a crate.** It is about 70 lines of codec over the lib0 `Read`/`Write` traits `yrs` already exposes: five message kinds (sync step 1 / step 2 / update, awareness, query-awareness) in a two-varint envelope. Crates exist (`yrs-axum`, `yrs-tokio`), but they are 0.x, they track `yrs` versions on their own schedule, and this is a wire format we cannot afford to be surprised by.

**Wire compatibility is the point.** Speaking the standard format means the client can use `y-websocket` as-is and we inherit reconnect-with-backoff, resync, and awareness timeouts rather than writing them. It also decouples the halves: either side can be replaced independently, which is exactly the optionality the [Frontend](#frontend--full-stack-rust) escape hatch depends on.

**The server is a participant, not a relay.** A scene that someone is editing is held in memory as a **`LiveScene`** — its own `Doc`, to which every update is applied. That is what lets a peer who was offline for a week be served by the server rather than by whichever other client happens to be connected, and it is the hook that persistence, compaction and search projections attach to. One `LiveScene` per scene, so the chain from scene to CRDT document is 1:1 the whole way down.

The "room" vocabulary the spike borrowed from `y-websocket` is deliberately **not** used: there is no room in this domain, only a scene that happens to have people in it. `LiveScenes::join(id)` hands out the one live copy, loading it from the store if nobody has it open.

**Awareness is relayed, never decoded.** The server treats awareness frames as opaque bytes and forwards them; it holds no presence state at all. Late joiners still get cursors, because on join the server broadcasts *query-awareness* and the connected peers republish themselves — an ordinary y-websocket client answers that automatically. This keeps presence genuinely ephemeral, exactly as [Presence is neither](#presence-is-neither) requires.

The known cost of that choice: **the server cannot retract a departed peer's cursor**, because it does not know which client ids a connection spoke for. Stale cursors linger until the client-side timeout (~30 s). The fix is to decode just the awareness header (client id, clock, state) and broadcast a tombstone on disconnect — small, and deliberately deferred.

Two things to revisit before this is production code: an unreadable frame is currently **logged and skipped**, which means a peer sending an unusable update diverges silently rather than being disconnected; and a `LiveScene` is never evicted, so it lives for the process lifetime with no backpressure and no auth on the socket. The eviction race and its intended fix are written up in [TODO.md](./TODO.md).

### The `scenes` feature

The spikes proved the technology. This is where it lands in the codebase.

**The feature is `scenes`; the aggregate is `Scene`.** It mirrors `projects` / `Project` exactly, so `SceneId`, `SceneStore` and `SceneService` all follow without anyone having to think. `text` was ruled out on collision grounds — `yrs::Text`, `Y.Text` and ProseMirror's text nodes all already exist here, and a `Text` aggregate beside them is a permanent "which one?". `prose` was the runner-up and survives as *vocabulary* (and as the CRDT root key), but it is a mass noun: it does not pluralise like every other feature, and `Prose` makes an awkward type. The word an author would recognize and the word that fits the codebase turned out not to be the same word, and the aggregate name is for us.

**A scene is its own aggregate because it has its own consistency model.** Structure is event-sourced with optimistic concurrency on a version column; a scene is a CRDT that merges without coordination. One cannot be nested inside the other without one of them being wrong. The feature split is not tidiness — it is the two-speed model made structural.

**A scene has its own id, and it is a pool of its own.** `SceneId` is a UUID v7 like every other identifier here — generated when the scene is created, derivable from nothing. A scene belongs to a project, carries its own title, and links to the ideas it draws on. Nothing else holds it; the outline only points at it.

**A scene keeps two version counters.**

- The scene's own version changes when its REST routes change something, that is its title or its links. This is the version that `If-Match`, the `ETag`, and the scene's retitle and link messages carry.
- The prose counts its updates separately. This lets the word count tell which state of the writing it has already counted.

Because the two are separate, typing never makes a retitle look outdated.

*Considered and rejected, back when a scene hung off a piece (the pool's single kind before [it split in two](#ideas-and-scenes-are-two-pools--decided)): making a scene's id equal its piece's id.* It was genuinely tempting — the scene becomes derivable, so bringing one into existence needs no coordinating write at all. Two things sank it:

- **It bakes "one scene per piece" into the key.** A piece plausibly wanted a body, a synopsis and author's notes (the Scrivener arrangement), or two drafts of the same scene. Any of those needs a discriminator, and we would have had to guess its shape up front. With an opaque id the semantics live in the link instead, and a link is far cheaper to extend than a key.
- **The risk it avoided was smaller than it looked.** The worry was a two-phase create across two aggregates with no shared transaction. But **ordering** fixes that, not identity: write the scene first, emit the reference second. A failure between them leaves an **orphan** — unreferenced bytes a sweep collects — not a **dangle**, which is a reference to something that does not exist. Orphans are benign; this is ordinary referential discipline rather than a new class of bug.

The independent id is what made the later split cheap: when ideas and scenes became two pools, scenes already had ids of their own, and nothing had to be re-keyed.

**The link to an idea lives on the scene.** This reverses the first design, where a piece held `SceneAttached { scene }`. Once there were two pools the rule became [*the relating side owns the link*](#ideas-and-scenes-are-two-pools--decided): a scene draws on ideas, so the scene stores them, and `ideas` never learns what a scene is. Linking and unlinking are published (`scene.idea.linked`, `scene.idea.unlinked`), which is how the appearances projection answers *idea → where it appears*.

- **A scene is made when there is writing for it.** In the writing view the first words typed into an empty section become a new scene, so sketching structure allocates no `yrs::Doc`s.
- **The client never computes a scene id — it asks.** The derivation, the ordering, the link: all of it stays server-side. This is what keeps the scheme changeable without touching a single client.

Cleanup across features runs through the **transactional outbox**. So it is eventually consistent. That is a deliberate part of the design, not a compromise. The cleanups are:

- Discarding an idea unlinks it from every scene (`unlink-discarded-idea`).
- Deleting a scene removes it from the outline (`detach-deleted-scene`).
- Deleting a project deletes its scenes (`delete-scenes-of-deleted-project`).

**Crate layout**, following the [feature anatomy](#feature-anatomy--the-onion):

```
features/scenes/
├── contract/          nearly empty — the wire format is y-protocols, not serde
├── core/              Scene, the plain-text projection, the SceneStore port
├── adapters/
│   ├── store/         in-memory now, PostgreSQL later
│   └── sync/          the y-protocols codec, LiveScene(s), Peer, the socket
└── tests/
```

Three placements that are not obvious:

**`yrs` belongs in `core`.** This reads like a violation of *core has no frameworks*, and it isn't: the CRDT **is** the domain model of a scene, not an implementation of it. Swap `yrs` for Automerge and the merge semantics change — that is substance, not a swappable detail. The dividing line that matters: `axum` is transport, `sqlx` is persistence, `yrs` is the thing itself.

**The wire protocol stays in `adapters/sync`.** Core exposes intent — *catch up from this state vector*, *apply this update* — and the adapter does the message framing. Same rule as `rest` mapping DTOs with free functions, and for the same reason: a wire format has no business in the domain.

**Protocol dispatch belongs to the adapter, not to `core`.** The spike suggested a `Room` in core owning "the room's semantics", and building it showed that was already `core::Scene` — `apply`, `changes_since`, `state_vector` *are* the document operations. What is left is deciding, per message, who needs to hear about it, and that is wire-shaped: it lives in `adapters/sync` as `LiveScene::receive`, returning a `Reaction`.

`Reaction` names **three destinations** — `to_sender`, `to_others`, `to_store` — which is what keeps two policies honest and testable rather than accidental: a read must not write (catching a newcomer up stores nothing), and presence must never be persisted (awareness relays and nothing more).

**Per-connection state is a `Peer`, and its lifetime is RAII.** A `LiveScene` is shared by every peer; a `Peer` is one connection — its id, its private outbound channel, and its two pump tasks. `Drop` aborts them. That matters because the shutdown ordering is otherwise load-bearing and invisible: the outbound channel closes only when *every* sender is gone, and missing one leaks a task for the process lifetime. Encoding it in `Drop` makes the mistake unavailable rather than merely documented.

**Awareness never reaches `core` at all.** It is relayed as opaque bytes and it lives and dies inside `adapters/sync`. Presence is ephemeral by decision; this is that decision expressed as a dependency arrow.

## Ideas, scenes and views — the non-linear model

Weaveling is text in many views, not one hierarchy with prose hanging off it. This section settles what ideas and scenes are, why the outline is a view rather than the model, and what each view is allowed to own.

### The tree is a view, not the model

The obvious model is a tree of nodes with prose attached, and it is wrong here. An earlier, abandoned implementation of this same idea built exactly that, and the shape of its failure is what argues against it:

- Every node needed a parent at the moment of creation — the aggregate held a mandatory root and the add command required a parent id. There was **no place to put an unplaced idea**. Top-down planning was not the default; it was the only option.
- The node carried `expanded`, with a `NodeToggled` event in the stream. Whether a twisty is open — per-user, ephemeral, view-local — was recorded in the book's permanent history. That is the structural consequence of the tree being both model and view: one aggregate, so everything lands in it.
- The only reordering primitive was "swap two nodes", guarded so a parent could not be swapped with its child. No author thinks *swap these two scenes*; they think *move this after that*, or *promote this scene to a chapter*. Swap is what is cheap when children are a list and one aggregate owns the whole map — mechanism dictating domain vocabulary.
- Its structure module was ~3,300 lines. Its timeline was an empty page. There was no prose anywhere. It built the container thoroughly and never reached either the content or the second view.

The fix is a single move: **position is not a property of what is arranged.** Parent, order, timeline placement and board coordinates all belong to the *view* that arranges ideas and scenes, never to the thing arranged.

### Ideas and scenes are two pools — decided

**Decided 2026-10-01**, after a year of the single-piece model and one complaint that would not go away, and built in [M12](./ROADMAP.md#milestone-12--ideas-and-passages) (where scenes were still called passages). Until then the pool held one kind of thing, a **piece**. This is the record of why it became two.

**A piece was doing two jobs and they had different lifecycles.** A `Piece` was both a pool-item and a prose container. An **idea** — a character, a what-if, a scrap of dialogue, a title — is disposable: grouped, regrouped and thrown away, and losing one costs an afternoon's thinking. A **scene** is the book: versioned, synced through a CRDT, exported, and losing one is unforgivable. Different durability, different editing model, different value. That is what makes them two pools rather than one type with a flag.

**It also explains a complaint that was treated as an interaction problem.** Attaching pieces to outline sections was called *"very weird"* during [M10](./ROADMAP.md#milestone-10--the-outline) and redesigned twice without the feeling going. It was the model surfacing: attaching an *idea* to a section is a category error, because an idea is a note about what the content should be, not the content. No amount of drag-and-drop fixes that.

**Every link is owned by the relating side, never by the idea.** This is [*position is not a property of what is arranged*](#the-tree-is-a-view-not-the-model) widened from position to every relation. The timeline stores which moments an idea sits at — many per idea, its table, its problem. The cast stores presence. A scene stores the idea that prompted it. An `Idea` is an id, a name, and perhaps a kind and a description, and **it does not change when a view is added**. The alternative — an idea carrying links to sections, moments and threads — makes the one thing that should never change into a junction table that every new view has to edit.

**"Where does this idea appear" is a read model.** An appearances projection (the `appearances` feature) subscribes to the events of every view. It answers *idea → where it appears*. Because it is a projection, it can be rebuilt, and no write path owns it. A new view joins by publishing its events. Nobody has to depend on the new view. The projection also lets an inspector make one read. Without it, the inspector would ask five features a question that four of them would answer with nothing.

**Word counts are a read model too: `statistics`.** A scene does not need to know its own word count. The writing view needs it, and later the history of the work will need it. So the live scene counts its text with `libraries/words` and publishes `scene.words.counted { scene, project, words }`. The `statistics` feature stores the counts per project. This keeps the scene table about the scene only. Statistics can add counts over time without touching scenes. And the view loads the counts from a light route instead of opening every scene.

**Editing is orthogonal to ownership.** Selecting an idea on the board and setting a moment or a character from there writes to the timeline and the cast — the board learns nothing and owns nothing but placement. That is the [frontend join](#the-board-renders-through-a-frontend-join) widened, and it is a UI convenience that the model neither grants nor forbids. The board must stay usable with every inspector blank forever: its value is pre-verbal, and an inspector full of fields invites filling them in before the thinking has happened.

**The outline arranges scenes**, exactly as the board arranges ideas. This is what keeps [*the outline holds structure, never content*](#the-outline-arranges-sections-not-scenes) true rather than breaking it — scenes are already a pool with their own ids and their own feature, so a section referencing one contains nothing.

**The cost, and the thing to watch.** A view that can arrange either pool needs a tagged reference — `Idea(id) | Scene(id)` — rather than two parallel sets of code. That is cheap, but a reference-to-anything is how a model turns to soup, so each view says which kinds it accepts rather than accepting all of them by default.

**What is still unknown** is ergonomics, not structure: how often an idea maps one-to-one onto a scene, and what the timeline wants to hold. Those inform the views. The structural question was settled on lifecycle, which is why it did not wait for them.

### Ideas and scenes are the pools; views arrange them

An **idea** is an id, a title and the project it belongs to: a character, a what-if, a scrap of dialogue. A **scene** is a unit of the book: an id, a title, its prose and the ideas it draws on. Neither knows where it sits.

Each view is its own feature, with its own aggregate and its own invariants:

| view | arranges | by | invariants |
|---|---|---|---|
| `board` | ideas | free 2D placement | none — an infinite surface |
| `outline` | scenes, with ideas as notes beside them | sections that nest | acyclic; a scene sits in at most one section |
| `timeline` | ideas and scenes | temporal relation | orderable, nestable buckets |
| `threads` | ideas and scenes | named sequences | one may be in many |
| `cast` | ideas and scenes | character presence | many-to-many |

**Not a generic graph with typed edges.** That is the tempting generalisation, and it destroys the thing that makes each view useful: an outline that cannot express "acyclic" is not an outline, and no schema can help a traversal whose meaning lives only in the UI. Five modest features that each answer one question well beat one abstraction that answers none.

**Neither needs a view at all.** An idea can sit in its pool unplaced, and a scene can have prose written and be in no section. The board is the workshop; the outline is the manuscript.

**Arrangement is shared, viewport is personal.** Where a scene sits in the outline is part of the book and belongs in an event stream. Whether *your* twisty is open, or where your board is scrolled, never is. That is the direct lesson of the `expanded` field above.

**One view is privileged.** A book is ultimately linear and export needs an order, so the outline — not the board — is what "the manuscript" means. Better accepted deliberately now than discovered at export time.

### Naming

Boring nouns for data, place-names for views. An **idea** is an idea and a **scene** a scene wherever they appear; `board`, `outline`, `timeline`, `threads` and `cast` are places an author goes. Rejected: `card`, because a card is how the *board* draws an idea and a row is how the outline draws a scene — the name would describe one view's rendering. `node` and `graph` are both mechanism words, the same mistake as `tree` one level more abstract. `piece`, the first name for the pool, went when the pool split in two; `passage`, the first name for a scene, went in [M13b](./ROADMAP.md#milestone-13b--passages-become-scenes).

### The outline arranges sections, not scenes

The board was allowed to be simple: a card *is* an idea, so the board stores `idea -> point` and nothing else. The outline is not, and the reason is the table of contents. **A section carries its own title**, because what belongs in a table of contents is rarely the working title of the idea the scene grew from. And **the structure of a book is not the structure of the ideas it came from** — an author plans Part Two before knowing which of their scenes will fill it.

So a **section** is its own thing: an id, a title, children, and what is attached to it — scenes, and ideas as notes beside them. A section may hold several scenes, and a section that holds none is perfectly legal.

**This is not the tree-as-model that [was rejected above](#the-tree-is-a-view-not-the-model), and the difference is worth stating** because the two look alike from a distance. The abandoned implementation's first failure was that *every node needed a parent at the moment of creation*, so there was nowhere to put an unplaced idea. That cannot recur here: scenes live in their pool and the outline is optional. A scene may have prose and be in no section at all — it simply is not in the book yet. What the outline holds is structure, never content.

**The outline is its own root, and that root is never materialised.** A section's parent is `Option<SectionId>`, where `None` means *directly inside the book*, and the outline holds those top-level sections as its own `children` — the same word `HeldSection` uses, because it means the same thing one level up. There is deliberately no root `Section` standing for the book. The rejected design had one, and it is precisely what made every node need a parent at creation; a materialised root is the mandatory root wearing a different hat. Keeping it implicit is what lets an outline be empty, and lets a section be added with no parent named.

**The trap that design did fall into is still open, though.** Its structure module reached ~3,300 lines and it never got to prose. Sections cost more machinery than "order the scenes" would have, so the guardrail is a hard one: a section has an id, a title, children and attachments, and **nothing else**. No `kind` or `level` field — depth in the tree is the level, and a `kind` would immediately start refusing structures an author wanted. No numbering, which is a rendering. And above all **no expansion state**: `NodeToggled` in an event stream is named above as the structural failure of tree-as-model, and it is the single easiest mistake to repeat here.

**An empty leaf is a warning, never a refusal.** A section with no children and no scenes is a hole in the manuscript, and the view says so — but the aggregate accepts it without complaint. Planning is exactly the act of writing down a structure you have not filled yet, so refusing would break the feature's main use. The same instinct as [`IdeaTitle` permitting the empty string](#the-event-catalogue).

**An unnamed section is just unnamed.** `SectionTitle` permits `""` so an author can sketch structure before naming it. This once went further — a view drawing an unnamed section that held exactly one piece showed that piece’s title — but that died with the split into two pools ([M12](./ROADMAP.md#milestone-12--ideas-and-passages)): a section now holds scenes, and a scene carries its **own** title, so the name of a scene lives on the scene, not on the place it sits. An unnamed section reads as *Untitled*, and that is the prompt to name it.

**Two orderings, not one.** Sections nest and are ordered among their siblings; scenes are ordered within their section. The manuscript is the depth-first walk: at each section, its own scenes, then its children. That is what `reading_order()` returns, and it is what export will mean.

**Moves are shaped like intent, and one of them is not obvious.** `Move { section, under, after }` is what dragging produces. `Promote` and `Demote` are the outliner's Shift+Tab and Tab, and they exist separately so the audit log records what the author *did* rather than where the section landed — which is also why their events carry only the section id: an event carries the command's parameters and never anything derived from state.

**Promote takes the sections that followed it along as children.** This is the behaviour of Word, Workflowy and Logseq, and here it is load-bearing rather than a convention: the alternative silently reorders the book. Lifting a middle child out while its later siblings stay behind means text that read *Rain, Storm* now reads *Storm, Rain*. Adopting them keeps the reading order byte-identical, which is the property a manuscript cannot afford to lose. There is a test asserting exactly that.

**Promote at the top and demote at the front of a list emit no events.** There is nothing to lift out of, and nothing to nest under. A command that changes nothing produces nothing, the same as [`Reshape`](#the-event-catalogue) emitting only for what actually moved.

**Removing a section lifts its children into its place and returns its scenes to the pool.** Neither is deleted. Pruning a chapter is a structural edit, and it must never be a way to lose prose — the scenes simply stop being in the book, which is where they started.

### The event catalogue

**`ideas`** — one aggregate per idea:

```
Captured  { project, title }
Retitled  { title }
Discarded { }
```

**`boards`** — one aggregate per board:

```
Started      { project }
IdeaPinned   { idea, at }
IdeaMoved    { idea, to }
IdeaResized  { idea, to }
IdeaRaised   { idea }
IdeaUnpinned { idea }
```

Pin, move and unpin are corkboard words, and they keep *the board's arrangement* clearly separate from *the idea exists*.

**`outline`** — one aggregate per outline:

```
Started         { project }
SectionAdded    { section, under, after, title }
SectionRetitled { section, title }
SectionMoved    { section, under, after }
SectionPromoted { section }
SectionDemoted  { section }
SectionRemoved  { section }
Attached        { attachment, to, after }
Detached        { attachment, from }
```

Attach and detach are the outline's words for *this scene is in the book here*, kept clearly apart from pin and unpin, which are the board's. An attachment is tagged as either a scene or an idea. If something is attached while it already sits somewhere else in the outline, it simply moves to the new place. There is no error, and export can never output the same text twice.

**A title may be empty, and that is a value rather than a missing one.** An idea captured on the board starts as `""` and the author types into it. So `IdeaTitle` mirrors [`ProjectName`](#identifiers) in every rule but one — it trims, rejects control characters and caps length, and it **permits the empty string**. The asymmetry is deliberate and worth knowing before someone tidies it away: a project is named by an author who has already decided to start one, while an idea exists precisely so that a thought can land *before* it has a name.

Untitled is stored as `""`, never as `Option<IdeaTitle>`. Two representations of the same absence would force every reader to handle both `None` and `Some("")`, and trimming already collapses a whitespace-only title into that one form.

**"Untitled" is a rendering, never a value.** A view draws a placeholder for an empty title; it does not store one. Storing the word would make an author who actually typed "Untitled" indistinguishable from one who typed nothing at all.

**Moves commit on drop, not during the drag.** Free placement means dragging, and a drag that emits events writes thousands of records per session, which would make the audit log unreadable — defeating the reason for event-sourcing the board in the first place. In-flight positions travel as **awareness**: ephemeral by construction, timed out on disconnect. The same division as prose, one level up:

| | ephemeral | durable |
|---|---|---|
| prose | cursors (awareness) | text (CRDT) |
| board | drag in flight (awareness) | placement (events) |

The cost is honest: the board needs its own live channel, since awareness in our stack is bound to scenes on `/api/scenes/sync` and a board is not a scene. Two live channels carrying different things, unable to share a protocol.

**Moves must not conflict.** Moves of *different* ideas commute, so `IdeaMoved` takes no strict version check, and concurrent drags of the *same* idea are last-drop-wins. Rejecting a move with "someone else moved it, reload" is the wrong answer for a position — no work is lost, the idea simply lands where the last author dropped it. Optimistic concurrency is reserved for the commands where a conflict means the author's intent no longer applies: pinning and unpinning.

### The board renders through a frontend join

The board stores `idea -> point`; a corkboard shows words. Because `ideas` and `boards` are separate features and [features may not depend on each other](#dependency-rules), titles arrive by the client fetching both and joining them — the legal move the dependency table already names.

This has a property worth stating outright: **the join is the tolerant read model.** A placement whose idea is gone has nothing to draw. Discarding an idea leaves a placement dangling until `unpin-discarded-idea` collects it, and the client must not depend on that having happened — it resolves at render time. The price is that the board may briefly disagree with the pool, which is acceptable for an arrangement and would not be for prose.

### Board creation: find-or-start

Multiple boards per project are supported by the model from the first event; the first version ships one. The rule that keeps that cheap: **a `BoardId` exists from the start, and every board event is addressed to it.** The query is "which boards does this project have?" — a list that happens to hold one element, never a `boardFor(project)` singleton. Keying placements by project id is the shortcut that turns a second board into a migration of every event ever written.

A project cannot create its own board: a feature may not call another feature. So a board is started **on first open** — find-or-start, inside `boards`, under one write guard exactly as [the in-memory scene store does it](#how-an-in-memory-backend-is-atomic). The invariant "a project has a board" is enforced nowhere; it is simply true by the time anyone reads.

The consequence to remember: **a project may legitimately have no board yet.** Anything sweeping a project — deletion above all — must tolerate its absence rather than assume it.

## Local-First

Because prose is a CRDT, the client keeps a **full replica in the browser (IndexedDB)**. This gives us, almost for free:

- **offline editing** — the server being down or unreachable never blocks writing,
- **zero typing latency** — edits apply locally and sync in the background,
- a concrete expression of the "your data is yours" value.

The client is designed to be **local-first-capable**, not a thin dumb terminal.

## Two Ways to Run It

Weaveling ships in two shapes, and the difference is not a build flag on one product — it is which adapters the composition root reaches for.

**Served** is the shape everything above assumes: a durable store, a broker once there is a second deployable, several authors on one project.

**Local** is a single author on their own machine with **no database and no broker**. Every port takes its in-memory adapter, and a project survives by being **exported to a file** and imported at the next start. That is a *project* export — a project made portable — and has nothing to do with [manuscript export](#export), which makes a project readable.

**Local is single-user by definition.** No accounts, no authors on other machines. Two browser tabs on the same computer still collaborate, because the sync socket and awareness are running locally and know nothing about deployment — that falls out of the design rather than being a feature to build, and it is worth keeping rather than suppressing: an author with a manuscript open beside a research window is the same person twice.

### What this does to the in-memory adapters

They stop being scaffolding. `InMemoryEventStore`, `InMemoryIdeaCatalog`, `InMemorySceneStore` and `InProcessDispatcher` become **production code for one of two modes**, which is why every one of them is judged by the same conformance suite as its durable counterpart. The suites were written as a hedge against a database swap; they turn out to be the thing that lets a mode ship without one.

It also **removes an argument that was used again and again to postpone work**: *"it is in-memory, so it dies at process restart"*. That is true for a test run. It is false for an author who works all afternoon and exports at the end. Three things were harmless only as long as a session was short:

- the [dual write in the idea catalog](./TODO.md),
- the deletion cascade,
- scenes that are never evicted.

Once a session is expected to last, each of them has a real cost. Each is recorded with that consequence noted.

### Export is what makes it persistent, and it proves the events

An export carries the projects, every aggregate's **event streams with their metadata**, and each scene's CRDT state — the last of which `Scene::everything()` already produces, since a `yrs` update from nothing is exactly a full document.

It deliberately does **not** carry the idea catalog. That is a **choice, not a constraint**. Derived data could well be exported, to warm the cache on import. Leaving it out gives a smaller file. It also makes it impossible to import a catalog that disagrees with the events. The cost is a rebuild pass on import.

That pass needs no new machinery. The export already holds every aggregate's event stream. So **the importer itself provides the list of streams**, and it can feed each stream through the projector while it reads. This point matters: it means local mode is *not* what forces a general rebuild-from-events. [That debt has a different trigger](./TODO.md).

### Who reads a refusal differs, the flow does not

Every mode runs [one flow](./ROADMAP.md#milestone-11b--one-flow-in-every-mode). A refused message is retried with a growing backoff. When the attempts run out, it is set aside as a [dead letter](#messaging--the-seam-now-the-transport-later). What differs is who reads the dead letter. A server has someone who runs it. Local mode has no ops team, but the person who caused the failure is sitting right there. So the client shows the author what was refused. The author can try again or dismiss it.

Who reads it is a **deployment matter, never something a feature reads**. `Publisher` only returns the one error that a broker can truthfully report. The reason is that a feature must behave the same way in both modes. Publisher confirms only say that the *broker* accepted a message. Consumer acks go to the broker and do not come back to the publisher. Anything richer would be a guarantee that only works in-process. Having the local mode makes this rule stricter, not looser: the code has to work in both modes.

## Wiring — each feature assembles itself

`app()` used to build every service by hand.
It decided which ports each service takes, which listener to register, and which routers to merge.
With three features, this was thirty-five lines long.
It named `ProjectService`, `SceneService`, `LiveScenes`, `IdeaService`, `Publishing` and the catalog projector.
So adding a feature meant editing the composition root, and it meant knowing the inside of the feature.

Every feature now has a **`wiring` crate**.
It sits next to the `contract`, `core`, `adapters` and `tests` crates.
It takes the feature's ports and returns what the feature adds to the application.
`app()` names no service and no adapter.

**Every feature is wired to the same shape, so the root can loop.**
`libraries/wiring` holds two plain structs:

- `Context { clock, publisher }` is the infrastructure that every feature receives.
- `Wired { routes, listeners }` is what every feature gives back.

Each feature has a function `wire(&Ports, &Context) -> Wired`.
`app()` names each feature exactly **once**:

```rust
let features = vec![
    projects_wiring::wire(&adapters.projects, &context),
    scenes_wiring::wire(&adapters.scenes, &context),
    ideas_wiring::wire(&adapters.ideas, &context),
    boards_wiring::wire(&adapters.boards, &context),
];
```

Merging the routes and registering the listeners are then loops over that list.
Before this, each feature was named three times:

- once to wire it,
- once in the listener chain,
- once in the router merge.

So adding a feature meant three edits in three places.
Forgetting the middle one would have silently dropped the feature's projections.

**This was deliberately not a `Feature` trait — until a third backend.**
A trait would have given us dynamic dispatch, which we had no use for.
A shared struct gave the same uniformity without `dyn`.
*Revised on 2026-10-07, in M13 step 0b:* by then every feature was named about ten times in `services/api`.
It was named in its `Ports` (twice), its database, its `lay_out`, its `wire`, and its outbox in `outboxes()`.
Forgetting the outbox still compiled, but the feature's messages silently stopped.
Local mode would have added a third round of each of these.
See [below](#one-value-per-feature).

`wiring` is a third *join* library, for the same reason as [`eventpublishing`](#the-first-consumer-the-idea-catalog-as-a-projection) and `serving`.
`wiring` is the place where `axum` meets `messaging`.
Only the wiring crates and the composition root live at that meeting point.
The reference implementation puts this in the same place: per-feature `*-starter` modules, which the application assembles.

Every feature returns the same type, even one without listeners.
**Uniformity is the point.**
A feature that later gets a listener adds `.listening(…)` and does not change its signature.
(When this was written, `projects` and `scenes` had no listeners. Since 2026-11-13 every feature keeps its own access read model, so every feature listens.)

**Every wiring returns a `Wired`, holding exactly what that feature contributes:** its routes, and where it has them, its listeners, chores and live topics.
A struct is better than a bare `Router` for two reasons.
The call site reads the same for every feature.
And a feature that later gets listeners or chores adds them and does not change its signature.

**Listeners are returned, not registered.**
A feature could take the dispatcher and call `listen` itself.
But that would tie the feature crate to `InProcessDispatcher`, a concrete type the feature should not know about.
When the feature hands its listeners back, the root decides what they are attached to.
This is the same reason why `Publisher` arrives as a port and not as a dispatcher.

**A feature may depend on another feature's `contract`, and only that.**
`boards_messaging` reads `ideas_contract::{DISCARDED, IdeaEventDTO}`, so that it can subscribe to ideas being discarded.
This is the first dependency between two features in the codebase.
It is also the moment where the messaging seam proves its worth.
It is allowed because [`contract` is a shared kernel](#feature-anatomy--the-onion): it depends on `serde` and nothing else, and it has no behaviour.
Boards gets the published routing key and the wire shape, which is exactly what a subscriber is entitled to.
Boards gets no access to the domain, service or store of ideas.
The [feature → feature ban](#dependency-rules) exists to stop features from *calling* each other.
Reading a published contract is the opposite of a call.

What a subscriber must not do is reach back for data that the message did not carry.
Boards needed to know *which board* holds a discarded idea.
The tempting fix was to add `project` to the discard event.
This was rejected for two reasons.
An event carries what changed, and the project did not change.
And a publisher must not learn what its subscribers need.
Instead, boards answers the question from its own events, with a second projection that maps each idea to its boards.

**The composition root keeps the joins.**
Work across features cannot come out of a list of independent modules.
Examples are the deletion saga, and later the board reading the pool.
Wiring covers the routine assembly.
Anything that spans features stays in `app()` on purpose.

**Each feature declares what it needs, so the signature stops growing too.**
A `wiring` crate exposes `Ports`.
For example, `boards` needs an event store and a catalog, and `projects` needs one store.
`app()` takes a single `Adapters`, which holds one named field per feature.
This fixed a real problem, not just a smell.
Clippy's `too_many_arguments` lint fires at eight arguments, and we build with `-D warnings`.
The seventh port was already in place.
So the next feature that needed a port would have **failed the build**.

The bigger point is not the number of arguments.
It is that `app()` no longer knows *what* a feature needs, only that it needs its ports.
This moves knowledge in the same way as the `wiring` crate itself.
Today every adapter needs nothing but its own constructor.
So `Ports::in_memory()` reduces the two call sites in the composition root to a short list:

```rust
app(Adapters::in_memory(Arc::new(SystemClock)))
```

At M11 this becomes a per-feature `Ports::backed_by(&pool)`.
It is chosen feature by feature, not in one god-function.
This puts the in-memory adapters into the production dependency graph.
That is correct, because [local mode](#two-ways-to-run-it) ships them as production code.

The [ideas test harness](./features/ideas/tests/src/wiring.rs) calls the real `ideas_wiring::wire`.
So the API tests exercise the production wiring.
They do not use a parallel copy that could drift away from it.

### One value per feature

**A feature is now a type that implements `wiring::Feature`.**
The service names it exactly once:

```rust
let features = vec![
    assemble::<ProjectFeature>(&storage, &context).await?,
    assemble::<SceneFeature>(&storage, &context).await?,
    // …
];
```

The trait states what only the feature knows:

- its `NAME`,
- its `Ports`, in memory and on PostgreSQL,
- its schema, as a list of migrators,
- its outbox, if it has one,
- how to `wire` it.

`wiring::assemble` does the steps that every feature used to repeat:

- It picks the ports for the `Storage`.
- It prepares the feature's database.
- It lays down the feature's schema.
- It returns an `Assembled { outbox, routes, listeners }`.

The service reads all its outboxes, routes and listeners from that one list.
So **a feature cannot be half-registered**: its outbox comes with it, or nothing does.

**Still no `dyn`.**
`assemble` is generic over the feature.
So the trait is a compile-time contract with associated types, not an object.
The service keeps a plain `Vec<Assembled>`.
`Ports`, `wire` and `service` stay public next to the impl, because each feature's own wiring tests build on them directly.
*Since M13d*, a feature builds its ports inside `Feature` itself, from the whole `Context`.
Before, they were built in `Ports` constructors, and the trait only forwarded to them.
Also since M13d, the four event-sourced features take their event store and outbox from one `wiring::EventSourced`.
This is because that pairing was written out four times in the same way.

**The PostgreSQL methods have defaults** that return `Unprepared::NotBuiltIn`.
Cargo unifies features across a build.
So `wiring/postgres` can be on while a feature crate's own `postgres` is off.
If a trait method existed only under that feature, it would then be missing from the impl, and the build would fail.
With a default, a misconfiguration becomes an error at runtime instead.

**Where a database comes from is a port too.**
`Storage::Postgres` holds an `Arc<dyn Databases>`.
In production, `ServerDatabases` creates and connects `weaveling_{feature}`.
The PostgreSQL service tests instead hand out one fixture schema per feature.
They keep the pools, so a test can still look at a feature's rows.

## Transactions and Atomicity

> **A write is atomic inside one `Transaction` of one feature.**
> A driven port's write method takes `&mut Transaction`.
> Core begins the transaction.
> It writes through as many of the feature's ports as the use case needs, and then commits.
> A query needs no transaction.

*Revised on 2026-10-29.*
This rule used to say: "atomicity is a property of a single port method — never introduce a transaction object into a port".
That held as long as each write touched only one port.
A [transaction audit](./TODO.md) found the writes that touch two ports.
Examples are an invitation ticket together with the account's append, and a registry claim together with `Start`.
These writes were ordered so that a crash between the two steps should heal itself.
In several places it did not.
Every port of a feature lives in [that feature's own database](#dependency-rules), so one transaction across them is possible.
What was missing was a way to share that transaction.
[Why the old objections no longer hold](#a-transaction-in-the-port-signature--decided) is explained below.

What follows still holds for a write that touches one port.

**The aggregate is the transaction boundary.**
This is the orthodox DDD answer, and it is the right one.
An event store port is one method:

```rust
async fn append(&self, stream: StreamId, expected: Version, events: &[Event]) -> Result<(), StoreError>;
```

The version check and the append are atomic *inside* this method.
PostgreSQL uses a real transaction.
The in-memory backend holds its write lock.
A version mismatch is a `Conflict`.
The caller never learns which mechanism was used, because that is none of the caller's business.

**The outbox is an adapter detail.**
This is where people usually reach for transactions, so it is worth being blunt.
The port says *append these events*.
The PostgreSQL adapter appends them **and** writes the outbox rows, in one transaction.
It sends a notification on that same transaction.
A relay wakes up on that notification and later publishes from those rows.
The in-memory adapter does **not** get an outbox.
A crash there loses the events together with anything queued.
So an outbox would guard against a failure that cannot lose one without the other.
Instead, the in-memory adapter keeps publishing through the service.

**[M11b](./ROADMAP.md#milestone-11b--one-flow-in-every-mode) overturns this.**
The reason has nothing to do with durability.
Publishing inline means that local mode and server mode take different paths.
So the suite that runs in memory cannot see a bug that depends on a projection being one step behind.
So the two backends differ in where publishing lives.
Each feature's `Ports` constructor chooses the store and its publishing together.
This is what stops anyone from wiring a double publish, or a missing publish, by hand.
Nothing above the port knows which way it is done.

Work across aggregates of different features stays eventually consistent through the outbox, by choice.
An example is unlinking an idea from its scenes when the idea is discarded.
A transaction never crosses a feature line.
There is no transaction across two databases that we could use.

### A transaction in the port signature — decided

*Decided on 2026-10-29.*
*This replaces the earlier decision "No `Tx` in a port signature". Its three objections are answered here.*
`libraries/transacting` holds the type, and nothing in it names a database:

```rust
pub struct Transaction(Box<dyn Unit>);

#[async_trait]
pub trait Transactions: Send + Sync {
    async fn begin(&self) -> Result<Transaction, Failure>;
    async fn join<'a>(&self, within: Option<&'a mut Transaction>) -> Result<Joined<'a>, Failure>;
}

pub enum Joined<'a> {
    Own(Transaction),
    Borrowed(&'a mut Transaction),
}
```

```rust
let mut tx = self.transactions.join(within).await?;
self.tickets.take(&mut tx, digest).await?;
self.events.append(&mut tx, id, expected, &stream).await?;
tx.finish().await?;
```

- **The old objections.**
  - *Objection: "persistence vocabulary in core".*
    A transaction is a unit of work, not a database.
    Core never sees what is behind it: no sqlx, no connection.
  - *Objection: "threaded by hand, forever".*
    Accepted.
    Written out, it is only one parameter.
    And at every call, it shows which writes belong together.
  - *Objection: "the in-memory backend would fake it".*
    It does not; see below.
  - The closure form stays rejected, for the same reasons as before.
- **Core begins it, as a rule.**
  The use case knows what belongs together.
  It also knows what must stay outside the transaction: Argon2, a mail, or a sweep that handles each item in its own transaction.
  And only the owner of the boundary can retry it on `Outdated`, because the retry repeats the whole unit.
- **Core owns every write's transaction.**
  *Decided on 2026-11-01, to have one standard way instead of two.*
  Every core write method begins its transaction, writes through it, and finishes it.
  It does this even when it only appends once.
  It hands the transaction to the event-sourcing service as `within`.
  It does not let the service open a transaction of its own.
  When a command makes a snapshot due, the snapshot is taken in that same transaction, right after the command's events.
  *Revised on 2026-11-01, when it was built; the user chose this.*
  At first, an `after_commit` was decided for the snapshot.
  But a snapshot is an event with its own version.
  The command returns the snapshot's version.
  REST hands this version to the client, so that the client can aim its next edit at it.
  A followup runs only after core has its answer.
  So with `after_commit`, the answer would be one version behind the stream, and every hundredth edit would come back as a 412.
  In one transaction, the answer is right.
  Best-effort snapshots are also no longer needed.
  They existed because a snapshot that failed in its own transaction made a command that had landed look failed.
  The command was then tried again and appended twice.
  Now a snapshot that fails rolls the command back with it, so trying again is correct.
- **The facade owns it, not the service behind it.**
  *Decided on 2026-11-01. The user asked why a feature's service retried inside a core that began its own transactions.*
  Each feature is entered through one struct, its facade, such as `ProjectsCore` or `IdeasCore`.
  The facade begins every write's transaction, asks for permission before it, and retries.
  The feature's service behind it, such as `ProjectService` or `IdeaService`, holds no `Transactions`.
  It takes `&mut Transaction` in every write, in the same way that the event-sourcing service takes `within`.
  So a reader of a facade method sees the whole unit.
  And a method that later writes something next to the events joins the transaction that is already there.
  The number of tries a write gets depends on what it aims at.
  `eventsourcing::tries_for(expected)` returns one try for a write aimed at a version, because its conflict is the answer.
  It returns five tries for a write aimed at no version.
- **Every kept listener is heard through its inbox.**
  *Decided on 2026-11-02. The user chose two things:*
  *every `Delivery::Kept` listener over only those whose effects are not idempotent,*
  *and the type system over a convention, to make sure no listener is forgotten.*
  A kept listener is an `inbox::KeptHandler`.
  It receives messages through `inbox::Inboxed`, which does the following:
  - It begins the transaction.
  - It asks the feature's inbox `heard_before`.
  - It hands the transaction to the handler.
  - It finishes the transaction.
  If the transaction loses a race, `Inboxed` tries it again inside the process, through `transacting::retrying!`.
  *On 2026-11-02 the user first chose not to retry inside the process, because the delivery's retry seemed enough. That changed on 2026-11-04, when every PostgreSQL transaction became serializable and lost races became common; since then `Inboxed` retries like every other owner of a transaction.*
  A refusal by the handler itself is not retried here; the delivery offers the message again.
  The handler passes the transaction to core as `within`.

  A sweep and a fleeting listener stay plain `messaging::Handler`s, and they say so.
  `Handler::WITHOUT_INBOX` is either `WithoutInbox::Sweep` or `WithoutInbox::Fleeting`.
  The handler's delivery follows from this value.
  So nobody can write a `Handler` without making this decision.
  Every other kept listener is a `KeptHandler`, and it becomes a `Listener` only through `Inboxed`.
  A hand-written `Listener` may still go without an inbox, on purpose.
  *The user kept that power on 2026-11-02.*

  **A read that must see what a racing writer committed is a follow-up hearing of its own.**
  *Decided on 2026-11-02: the user chose a follow-up message over a lock on the data that the two writers race over.*
  Sometimes two racing writers each need a write and then a read, so that one of them always sees the other's write.
  The write and the read cannot share a transaction.
  Both writers could read before either of them commits.
  So the listener does this instead:
  - It writes.
  - In the same transaction, it enqueues a follow-up message to itself through `Outbox::enqueue_within`.
    The key of that message has `follow-up` as its first segment.
    No feature's wildcard covers that key.
  - The follow-up is delivered only after the write has committed.
  - The follow-up then reads and acts, in a separate handling of its own.

  This is how boards and outline remove a discarded idea or a deleted scene.
  It is also how invitations withdraw an invitation into a deleted project.
  There, the sweep itself waits for the follow-up of the tombstone.
  No separate transaction is begun under `Inboxed`.
  It would wait forever on SQLite's single writer.
  The wrapper lives in `inbox`, not in `messaging`, because `inbox` already depends on `messaging`.
- **Whoever owns the transaction retries it.**
  *A kept listener is the exception: its delivery retries it (2026-11-02).*
  A write that loses a race (`Outdated`) is tried again as the whole unit, in a fresh transaction.
  It is tried up to five times, with a growing, jittered pause between tries.
  This goes through one helper in `transacting`: `retrying!(transactions, |transaction| { … })`.
  The helper runs its block in a fresh transaction for each try.

  *It is a macro, not the async closure that was chosen first (2026-11-01).*
  A future that awaits an async closure which borrows anything, above all `&self`, is not `Send` on stable Rust.
  The compiler says "implementation of `Send` is not general enough".
  axum and the messaging library need `Send`.
  The macro writes the loop out where it is used.
  So the block reads like the body of a closure.
  `?` and `return` end the current try.
  And the block can borrow whatever it likes.

  As a rule, the owner is core, so retrying lives in core and not in an adapter.
  There are three reasons:
  - Only the use case knows whether a retry is safe.
    A command aimed at an expected version is never retried.
    Its conflict is the answer, which is a 412 over REST.
  - REST is not the only caller.
    Listeners and chores call the same methods.
    And messaging's delivery retries are far too coarse for writes that collide within a millisecond.
  - What a use case does outside its transaction, such as a mail or a live frame, must not happen again on a retry.
    Only a retry inside the use case can prevent that.

  An inbound adapter that passes in its own transaction owns the unit, and so it retries the unit, with the same helper.
  The listener with an inbox is such an adapter.
  REST, listeners and chores never retry a conflict themselves.
  They only map the answer that core returns.
- **An inbound adapter may pass a transaction in when the use case needs it.**
  The case that asked for this is the listener whose inbox row must commit together with the handler's effects.
  This is not a rule either way.
  The transaction stays in core unless there is a reason.
- **One core method, not one per caller.**
  A core method that may join a transaction takes `within: Option<&mut Transaction>`.
  Once the method has an options struct, this is a field of that struct instead.
  `join` either returns a transaction of its own or borrows the caller's.
  `finish` commits only a transaction that it owns.
  REST passes `None`.
  The listener with an inbox passes its own transaction.
- **Strict at the port, lax in core.**
  A driven port's write method always takes `&mut Transaction`.
  There is no pool variant and no `Option`.
  A forgotten transaction would silently lose atomicity, and the compiler is what prevents that.
  A read takes a transaction only when the read decides a write.
  Examples are the tombstone check before `remember`, and a `FOR UPDATE`.
  A core method that writes only once still opens its own transaction, as described above.
  This costs one round trip.
  Things that live outside the feature's database cannot join a transaction, and they take none.
  Examples are messaging's deliveries and the `Mailer`.
- **Explicit commit; dropped is rolled back.**
  A transaction is not committed on `Drop`.
  This was decided after asking the user.
  An early return through `?` would otherwise commit half the work: the ticket gone, but the append missing.
  Also, `Drop` cannot await, and it cannot return the error that a commit may fail with.
  So the caller would report success for a write that never landed.
  Instead, `Transaction` is `#[must_use]`.
  *Revised on 2026-10-31, when it was built:* a transaction dropped without a commit is not logged, as was first planned.
  Dropping is also how every use case that fails on a `?` rolls back.
  So the warning would appear with every ordinary error.
  A forgotten commit shows up as writes that never land, and the feature's tests see that.
- **The adapter downcasts** to its backend's transaction.
  For PostgreSQL this is `sqlx::Transaction<'static, Postgres>`.
  For SQLite it is SQLite's transaction, under `BEGIN IMMEDIATE`.
  The adapter does this through `transacting::postgres::connection(&mut tx)` and its SQLite twin.
  These hand it the connection, not the transaction, because only core ends a transaction.
  Handing one feature's transaction to another feature's port is a runtime error, not a type error.
  A marker type per feature was considered.
  It was not worth it for a mistake that is so unlikely.
- **In memory**, a transaction holds the feature's single async lock.
  So writers are serialised, in the same way as in SQLite.
  Each write registers its undo with the transaction.
  If the transaction is dropped without a commit, the undos run in reverse order.
  A reader outside the transaction can see a write that is not yet committed.
  This is accepted, because the in-memory backend serves only tests and development.
  The conformance suite proves what matters: a dropped transaction leaves nothing behind.
- **The outbox joins as it is.**
  `append` hands its transaction to `outbox::postgres::enqueue`.
  PostgreSQL delivers the `NOTIFY` only at commit.
  *Added on 2026-10-31, when it was built:* SQLite and memory have no `NOTIFY`.
  So a `Transaction` carries `after_commit` followups.
  They run once the transaction commits, and never when it is dropped.
  The in-memory outbox enqueues through such a followup.
  SQLite's outbox wakes its relay through one.
  So neither of them wakes a relay for a write that may still roll back.
- **The snapshot stays outside.**
  `carry_out` appends in the transaction it is given.
  The snapshot after it is taken after the commit, and it is best-effort.
  So a command that has landed is never reported as failed.
  *Superseded on 2026-11-01:* the snapshot is now taken in the command's transaction, as described in [Core owns every write's transaction](#a-transaction-in-the-port-signature--decided).

Until this is built ([M17, after slice 5](./ROADMAP.md#milestone-17--accounts-members-and-roles)), a write across two ports follows the interim rule: **the append is the commit point**.
The steps before the append may be done again by the same actor.
The steps after the append are driven by the event, or they are idempotent.

### How an in-memory backend is atomic

Take the lock **once**, validate, then mutate, all under the same guard:

```rust
let mut projects = self.write();
if projects.contains_key(&project.id()) {
    return Err(StoreError::Conflict(project.id()));
}
projects.insert(project.id(), project);
```

No caller can see a half-finished state.
An in-memory operation has no I/O that could fail partway through.
One discipline makes this hold: **never hold a `std::sync::RwLock` guard across an `.await`**.
Doing so blocks the executor, and the guard is not `Send`.

**What in-memory can prove:** observable atomicity.
A failed `create` leaves the store unchanged.
A version conflict appends nothing.
These checks belong in the **conformance suite**, so that every backend must honor them.

**What it cannot prove:** that a real backend actually wrapped its statements in a transaction.
In-memory tests pass happily while PostgreSQL runs two statements without a transaction.
There is no clever fix.
The mitigation is that such tests live with the backend.
That is already [how the test scopes are split](#repository-structure).

### A worked example: `SceneStore::apply`

`apply(id, update)` takes a delta and lets the backend choose how to store it.
The two possible choices have really different concurrency needs.
Both are handled entirely inside the method:

- **Snapshot backend:** it reads the bytes, merges the update, and writes the bytes.
  Two concurrent applies both read `S`.
  One writes `S+A`, the other writes `S+B`, and **one update is lost**.
  CRDTs do not help here, because the loss happens below the merge.
  This backend needs `SELECT … FOR UPDATE` or a version column.
- **Append-only log backend:** it does one insert.
  Appends commute, so concurrent applies need no locking at all.

Today this problem is hidden.
A `LiveScene` is the only holder of the document, so there is one writer per scene per process.
That assumption breaks as soon as a second server instance exists.
Either way, the handling stays *inside* `apply`.
That is the rule working exactly as intended.

## Event Sourcing — Discipline

- **Be selective.**
  Use event sourcing only where history has value, such as the structural domain and the audit log.
  Use plain state for settings, presence, search indexes, and blobs.
  "ES where history has value" — not everything.
- **Event store:** an events table with one stream per aggregate.
  It uses optimistic concurrency through a version column.
  Since [the store milestone](./ROADMAP.md#milestone-11--the-real-store), PostgreSQL backs it, with one database per feature.
  An in-memory adapter still passes the same conformance suite.
  Nothing above the port knows which one is used.
- **Every event carries its agent.**
  "Who did this" is part of the record.
  The slot has to exist from the very first event, because it cannot be filled in later.
  Anything written before the field exists stays anonymous forever.
  Until accounts exist, the slot holds a placeholder agent.
  That is cheap now and would be impossible later.
- **An event carries what changed, not what it replaced.**
  We write `Retitled { title }`, not `{ from, to }`.
  The previous value can always be recovered by replay.
  Anything that renders an idea's history replays that stream anyway.
  So it already knows the old title at the moment it needs it.
  Storing the old value too would put a second copy of one fact into an immutable log, and that copy could never be corrected.
  It would also shape one event unlike its siblings, for a reason that comes from rendering and not from the domain.
  One case would overturn this: a feed *across aggregates*, such as "everything that happened in this project today".
  Such a feed cannot afford to replay every stream it touches.
  If it arrives, the previous value is added through an ordinary event version bump and a patch.
- **Projections / async work:** a **transactional outbox** keeps projection updates reliable.
  Its relay is woken by `pg_notify` on the appending transaction.
  It polls only as a backstop.
  There is no message broker for now (see below).
  The outbox *model* belongs to `libraries/messaging`, not here.
  That model is the entry, the claim lock, the repo port and the relay.
  It belongs to messaging because it serves every published message, and a saga publishes commands, not events.
  Turning an aggregate's events into messages is a third, separate job.
  It lives in [`libraries/eventpublishing`](#the-first-consumer-the-idea-catalog-as-a-projection), which depends on both, and the outbox insert will hang there.
  What belongs to the event store is the **atomic insert**: appending events and inserting the outbox rows happen in one transaction.
  So the event store *adapter* depends on messaging, but `libraries/eventsourcing` itself does not.
  Either way, it stays invisible above the port — see [Transactions and Atomicity](#transactions-and-atomicity).
- **An inbox is the dual of the outbox.**
  It will be needed for the same reason: a message that is delivered again must not be handled twice.
  It was deferred together with the broker, but its shape is known.
  It remembers the message id and rejects messages it has already seen.
  *Built on 2026-10-31 as [`libraries/inbox`](./libraries/inbox/README.md):*
  `heard_before(transaction, listener, message)` lives in the feature's own database and runs inside the handler's transaction.
  It is used by the listeners whose effects are not idempotent on their own.
  The first such listener is `note-role-changed`.
- **Versioning / upcasting** is planned from day one.
  A book project lives for years, and its events will change.
  The mechanism is settled: **every event carries its own version**.
  Upcasting is a set of pure `from -> to` patch functions, applied when reading.
  So stored events are never rewritten in place.
  This was adopted from the earlier implementation.
  The design existed there and was sound, but it was never exercised.
  Every real event there stayed at version zero, and the only patch lived in the library's own tests.
- **Snapshots** for aggregates keep rebuilds fast at scale.
  A book with one paragraph per scene has thousands of scenes.

## Messaging — the seam now, the transport later

Features may not call each other.
So work across features travels as messages.
`libraries/messaging` owns the port, the envelope and an in-process dispatcher.
When a broker is wanted, it will be an adapter behind that same port.

**A broker does not solve the dual-write problem — it creates it.**
Committing an event to the store and then publishing it to a broker involves two systems with no shared transaction.
If the publish fails after the commit, the event happened, but nobody receives a message about it.
If the publish succeeds while the commit rolls back, we have published a lie.
The fix is the **outbox**.
The event row and the outbox row are written in one transaction, and a relay publishes from the outbox.
This is needed *whether or not* a broker exists.
The outbox is the part that carries the load.
A broker is only a transport attached to it.

This settles the order of the work.
The whole value of the outbox is that it is atomic with the event write.
The store is in-memory today.
So a broker now would relay from nothing durable, and it would lose messages on a crash.
That gives **weaker guarantees than a direct function call**.
At the same time, it would add a connection lifecycle, a policy for dead letters and poison messages, and containers in CI.

So what gets built now is the seam, because the seam is the part that is expensive to add later:

- the **port** — publish and subscribe, which features depend on and a transport implements
- the **envelope** — message id, the conversation it belongs to, what caused it, occurred-at
- an **in-process dispatcher**

The envelope is the urgent half.
**Correlation and causation ids cannot be filled in later.**
Messages written without them can never be traced.
This is the same argument as for the agent slot in [Event Sourcing](#event-sourcing--discipline).
The first real consumer is the project-deletion saga.
Its whole job is to answer "which of these deletes belonged to my request".

### No exchanges, but a named listener

RabbitMQ's routing has four moving parts: exchange, routing key, binding and queue.
Only two of them earn a place in the port.

**Exchanges do not.**
The reference implementation models `ExchangeTarget`.
After years in production, its `MessageTargetType` enum has exactly one variant.
More decisive: nothing there ever *chooses* an exchange name.
The only call site derives it: `ExchangeTarget.of(aggregateType.toLowerCase())`.
That means one exchange per aggregate type.
We already carry the aggregate type as the first segment of the routing key.
So an exchange here would be a **second encoding of a fact that the routing key already holds**, and two encodings can disagree.
So there is one durable topic exchange.
Its name is deployment configuration, and `Publisher` never sees it.
A topic exchange bound with `#` works as a fanout exchange.
Bound with an exact key, it works as a direct exchange.
So the other exchange types give us nothing either.

**The queue is what we were missing.**
In the reference implementation, a queue is identified by `{exchange}-{routingKey}-{listenerName}`.
We have every part except the last one.
That is why `Listener` now names itself.
The name is not decoration.
The inbox that makes redelivery safe is keyed by `listenerName + messageId`.
So **idempotency is per listener**.
Only the feature knows the right name.
Once a broker exists, the name is durable deployment state.
Renaming a queue either strands the messages it held or processes them again.
So `ListenerName` refuses anything that would not work as a queue name.
It allows only lowercase letters, digits and hyphens.

**A listener binds as many routing keys as it needs.**
`listens_to` returns a `Vec<Subscription>`.
A RabbitMQ queue carries several bindings, and one pattern is often the wrong shape.
For example, the pin index wants `board.idea.pinned` and `board.idea.unpinned`, but *not* `board.idea.moved`.
No single pattern expresses that.
`board.idea.*` also catches the move.
Until this was fixed, that cost one projection write per drag.
The provided `Listener::hears` combines the bindings with `any`.
So a message that matches two bindings still arrives **once**.
That is what a queue bound twice does as well.

**`Delivery` is the second thing only a feature knows.**
The reference implementation declares queues in two ways.
Our two upcoming consumers each want one of them:

| | queue | a missed message | ours |
|---|---|---|---|
| `Delivery::Kept` | durable, shared, dead-lettered | redelivered, then set aside | the idea catalog projector, the deletion saga — a miss corrupts the read model |
| `Delivery::Fleeting` | non-durable, auto-delete, one per instance | gone | the board's live channel — a browser that missed one refetches |

So the in-process dispatcher dead-letters a refused `Kept` message.
It only logs a refused `Fleeting` message.
Nothing will ever replay a `Fleeting` message, and a pile that nobody drains is worse than a log line.
`Kept` is the default: **losing a message has to be chosen, never inherited**.
This also settles the question for local mode.
`Fleeting` never dead-letters under either transport, so the board needs no branch per mode at all.

### The first consumer: the idea catalog as a projection

Capturing an idea does not write the catalog.
The core appends events, and each event is **published** through the outbox.
`CatalogueIdeasListener` keeps the listing up to date, through `IdeasCore::catalogue`.
(This was built in M8, when ideas were still called pieces.)
The vocabulary is deliberately plain: we publish events, we do not announce things.
The names say so: `message_for`, `IdeaEventDTO`.

**The message carries the whole event and its metadata**, not a curated summary.
The first version published only the id.
The reasoning was that the only subscriber read the state again anyway.
That was wrong, and the principle is worth stating plainly: **a publisher must not know what its subscribers need.**
If the payload is curated, every new subscriber becomes a change to the publisher.
That is exactly the coupling that messaging exists to remove.
Worse, it does not even work.
**Features may not depend on features.**
So a subscriber outside `ideas` cannot read our event store.
A message with only an id is usable only inside the feature that sent it.
The outline wanted titles and had no legal way to get them.

So a published message carries:

- the event's fields,
- the event's own **version** (each event is versioned on its own, so the field belongs to the variant),
- the aggregate id,
- the **version the event landed at**,
- the **agent**,
- and when it happened.

Dropping the agent was the worst mistake.
This document already says that the agent slot cannot be filled in later.
Yet the first payload silently made every message unable to answer *who did this*.

**What is internal stays internal, though.**
A snapshot is housekeeping, so it is **not published at all**.
`message_for` returns `Option<Message>`, and a snapshot is refused in two places:

- The library skips anything flagged `is_snapshot`. This is right for every aggregate.
- The feature's own mapping returns `None` for `IdeaEvent::Snapshotted`.

The service never hands a snapshot over anyway, because `Appended.events` excludes snapshots.
So nothing depends on luck.
For the same reason, there is no `is_snapshot` flag on the wire.
A subscriber that has to reason about our compaction would be coupled to our storage.

**The envelope belongs to a library; only the body belongs to the feature.**
`libraries/eventpublishing` owns everything that is the same for every aggregate:

- the routing key,
- the aggregate id and version,
- the agent,
- the timestamp,
- and refusing to publish snapshots.

For the event body alone, it calls a `fn(&E) -> Option<B>` that the feature supplies.
`message_for(happened, body)` is all there is to it.
The reference implementation draws the same line.
Its publisher builds the envelope and metadata generically.
For the body, it calls an `EventSerializer` that the developer supplies.

That crate depends on both `eventsourcing` and `messaging`.
That is why it is its own library and not a module of either one.
The reference implementation keeps it out of its eventsourcing *core* for the same reason.
The outbox that the messages go into became [its own library](#messaging--the-seam-now-the-transport-later), and the event store enqueues into it.

**The body is hand-written, in `contract`.**
`IdeaEventDTO` is a declared serde type in `ideas-contract`.
`adapters/messaging` maps `IdeaEvent` onto it in an exhaustive `match`.
Deriving `Serialize` on the domain event would have taken four lines, and it would have been wrong.
A harmless-looking refactor of the enum would silently change the published contract.
With a `match`, adding an event **fails to compile** until someone decides two things:
what the event says on the wire, and whether it is published at all.

**Routing keys are derived, and the declared ones are a guard.**
The library builds `{kind}.{event-name}` from metadata.
So a key cannot disagree with the event it describes.
The earlier hand-written `match` onto constants could drift, and it needed a test to catch that.
`contract` still declares `CAPTURED`, `EVERY_IDEA` and the rest, because subscribers need something to bind to.
Two tests assert that the derived key and the declared constant agree.
Both tests were checked by breaking them.

**Publishing is the store's business, so `core` sees neither `messaging` nor `eventpublishing`.**
The wiring hands the feature's `message_for` from `adapters/messaging` to the event store.
Every append enqueues one message per event into the outbox, in the same transaction.
This is what lets the routing keys and the payload live in `contract`, where a published contract belongs.
`core` may not depend on `contract`.
So as long as `IdeaService` did its own publishing, they were stuck in the wrong crate.
*Until M13d, the in-memory store published inline through an `IdeaEventPublisher` port.*
*Now every backend enqueues into an outbox, so that port is gone.*

**A subscriber may still read the state again, and ours does.**
`CatalogueIdeasListener` ignores the event fields in the payload and reads the idea back.
A *listing* wants the current state.
Reading again is idempotent.
A late redelivery still reads the latest state, so it cannot roll the listing back.
That is a choice about this projection, not about the message.
The first version mixed these two up.
A subscriber that needs history, not the current state, now has everything it needs in the payload.
It can guard on the aggregate's version.

**The service returns what it appended.**
`EventSourcingService` used to return only a `Version`, which cannot say what happened.
So `begin`, `execute` and `execute_at` now return `Appended { version, events: Vec<Recorded<E>> }`.
They return `Recorded`, not bare events, for two reasons:

- The metadata is half of what gets published.
- Only the metadata per event carries **each** event's version.
  A command with two events lands at n+1 and n+2.
  The final version alone loses the first one.

Deriving the routing key from the command instead would have been wrong in two ways:

- A command that changes nothing emits no events, for example retitling to the same title.
- One command may emit several events.

A snapshot written at the end is deliberately **not** in `appended.events`.
Compacting the log is not news.

**Reading a message back is a `*_in` family.**
`message_for` builds a message.
`event_in` and `idea_in` take one apart, using the library's `published_in`.
A listener names what it listens to with the contract's own constants, for example `LISTENS_TO = &[EVERY_IDEA]`.
The library parses them.
So no caller repeats the pattern or its `expect`.

**A listener is named for the job it does, not the thing it maintains.**
So it is `catalogue-idea`, not `ideas-catalog`.
The reference implementation settled on the same rule, with names like `colors.color-created-update-lookup` and `colors.user-deleted-remove-permissions`.
A feature grows several listeners, and a component name cannot tell them apart.
The convention is an **imperative verb plus its object**, lowercase and hyphenated.
`ListenerName` already enforces the format.
The name must be unique across the deployment.
It becomes a queue name and an inbox key, not just a label.

### Read-your-writes, once the listing is a projection

A projection lags behind by however long the transport takes.
In process, that is zero.
This makes it dangerously easy to write a client that silently depends on a lag of zero.

The write path was already safe.
`POST /api/ideas` and `PATCH /api/ideas/{id}` answer from the **event store**, not from the catalog.
They return the full idea and its `ETag`.
Only `GET /api/ideas` reads the projection.
So the fix belonged in the client, and the fix was to **stop asking**.
The pool used to fetch the whole list again after a capture.
Now it keeps the idea that the server just returned.
That is honest under either transport, needs no spinner, and saves a request.

The alternative was to keep the refetch and rely on the in-process dispatcher being synchronous.
That would have passed every test today.
It would have broken on the day a broker arrived.
And the symptom would have shown up in the UI, not at the seam.
The [ideas E2E suite](./clients/web/e2e/ideas.spec.ts) still covers the projection honestly.
Several of its tests reload the page or navigate back to the pool.
A pool that is mounted again reads the catalog and nothing else.

### At-least-once is the trap this design has to survive

The in-process dispatcher delivers each message exactly once, synchronously, and in order.
**RabbitMQ delivers at least once.**
It delivers again on a nack, on a dropped connection, and on a consumer restart.
Nothing we build before a broker exists will ever force a listener to be idempotent.
So every listener written until then will pass its tests and still be quietly wrong.

This mirrors a mistake that was already avoided on the publishing side.
There, an in-process dispatcher *could* report which listeners succeeded, but a broker cannot.
Both directions follow the same rule: **a feature must behave identically under either transport**.
So the port may promise only what both transports can keep.
Here, the in-process side promises too *much*.
The fix has two parts:

- the inbox, which dedupes on `(listener name, message id)` inside the handler's transaction,
- and a dispatcher that delivers messages again in tests.

That way, idempotency is exercised long before RabbitMQ can punish its absence.
Both parts were first [recorded in the TODO](./TODO.md). The inbox has existed since 2026-11-02, as described above, and that TODO entry is resolved.

A broker earns its place at the boundary this section always named.
That is a consumer deployed separately, or an async handoff across a real service edge.
RabbitMQ is the likely choice then.
Once fan-out gets complicated, its routing model beats anything we would write by hand.
Deferring it is cheap precisely *because* we already know it well.
Nothing above the port changes when it arrives.

## Frontend — Full-Stack Rust

We take the **full-stack Rust** path: Leptos, compiled to WASM.
It is part of the fun, and it keeps the whole stack in one language.

This choice was provisional until the editor spike settled it.
The worry was that a collaborative rich-text editor would be too heavy in Rust/WASM.
The fallback was to switch the frontend to **Angular**, with a proven editor plus Yjs bindings, and keep the Rust backend.
That escape hatch stays open, and keeping it open costs nothing.
The interop boundary is bytes, and the sync protocol is the standard one.
So neither half knows what language the other half is written in.
But nothing found so far argues for using the escape hatch.
Hosting ProseMirror from Leptos needs a boundary of about 20 lines, and the bundle cost is ordinary.
See [The client hosts the editor](#the-client-hosts-the-editor-rust-never-touches-the-document-model).

### Client conventions

**The builder, not the `view!` macro.**
Leptos offers both.
We use the builder by default (`html::div().class(..).child(..)`).
We use `view!` only where markup really dominates.
An inline SVG is the one current case.

The main reason is not tooling.
It is being honest about what the code *is*.
We counted a typical component.
About a third of a `view!` body was markup.
The other two thirds were Rust closures dressed in angle brackets.
So `view!` is an embedded DSL that mostly contains the host language.
That is the same failure as YAML with bash inside it.
The concrete harms follow from that:

- **`cargo fmt` ignores `view!` bodies entirely.**
  We verified this by mangling one body and watching `cargo fmt --all -- --check` pass.
  Every macro body in the client was unformatted and unchecked.
  With the builder, they are ordinary Rust.
- **Attributes become typed methods**, such as `.r#type("text")`, `.placeholder(..)` and `.disabled(..)`.
  A misspelled attribute is a compile error.
  `view!` accepts it silently.
- **rust-analyzer and compiler spans work properly** outside a proc macro.

Two consequences are worth knowing.
First, `#[component]` stays.
It does not depend on the syntax choice, and it gives us named props, called as `Foo(FooProps { .. })`.
Second, **`<Show>` is mostly no longer needed**.
It exists mainly because `if` is awkward inside `view!`.
In builder form, a plain `if` with `.into_any()` on both arms replaces it.
`<For>` is *not* in that category.
It does keyed reconciliation, which a plain `map` does not do, so we keep using it.

**The dividing line is optional props, not markup.**
`view!` is right for the route table and wrong for everything else we write.
The reason is mechanical, not a matter of taste.
The optional props of `#[component]` get their defaults only *through the macro*.
Building `XProps { .. }` by hand requires every field.
So the router's table takes 29 lines in builder form, against 12 with the macro.
The extra 17 lines are:

- `base: None`,
- `set_is_routing: None`,
- `transition: false`,
- two `ssr: Default::default()`,
- and two `ToChildren::to_children(move || ..)` wrappers, because the field of `RouteChildren` is private.

None of that carries meaning.
Our own components stay clean in builder form, precisely because **none of them has an optional prop** and their children are plain.
So `ProjectRowProps { project, workspace, overlays }` lists every field, and every field says something.
The rule: use the builder for components we own.
Use the macro when a third-party component brings optional props or wrapped children.

`leptosfmt` was considered and skipped.
With `view!` this rare, another tool in the chain costs more than it gives back.

**Every feature's stylesheet nests under its own root selector.**
`main.scss` concatenates the partials into one global sheet.
So a bare class name in one feature also applies to every other feature.
`_boards.scss` lives under `.board`, `_ideas.scss` under `.pool`, `_projects.scss` under `.projects`, and `_prose.scss` under `.prose`.
`_base`, `_controls` and `_overlays` stay global, because being shared is their job.

The rule was written after the third collision, and the third one is the argument for it.
A note had been filed about `.actions` leaking from the board onto the workspace.
One change later, a new `.surface` on the board silently inherited the prose editor's border, padding and `min-height: 8rem`.
This drew a ghost box behind the first card.
**None of it was caught by tests**, because the suites assert on roles and text.
The two tests that catch it now assert *layout*.
One checks that the board's surface has no box of its own.
The other checks that a project row still spreads its title, stamp and menu across its width.
That is the pattern to copy: check that a feature's chrome is still where it belongs, not only that it exists.

**Pure client logic gets unit tests, and they run without a browser.**
`cargo test --workspace` builds the client for the host and runs anything under `#[cfg(test)]`.
For a long time, the client was excluded from that and tested only through Playwright.
Anything that is arithmetic and not DOM belongs on the unit-test side of that line.
`viewport.rs` (the coordinate transform) and `carrying.rs` (where a drag or a resize lands) are both pure.
Together they hold 23 tests that used to need a browser, or did not exist at all.
A rule of thumb for when a view file grows: the part that is hard to test is usually the part that should be its own module.

**A mapping function's name says which way it goes: `as_*` reads a DTO, `to_*_dto` writes one.**
Both sides of the contract translate, and the types on both sides share a noun.
So a function called `spot` cannot be understood without opening it.
That was not hypothetical.
`fn spot` meant `SpotDTO -> Spot` in the boards REST adapter.
It meant `Spot -> SpotDTO` in the boards publisher.
That is the same name for opposite directions in one feature.

There is deliberately *no* dedicated mapping module.
On the client, `service.rs` is the only file that touches the contract at all, so the mapping already sits on the seam.
On the server, translating is the whole job of an adapter.
Moving the mapping out would empty out the thing that defines the adapter.
Rust's orphan rule settles the other tempting option.
Neither the DTO nor the domain type is local to an adapter.
So `impl From<SpotDTO> for Spot` is not possible there, and free functions are what is left.

**`LocalResource` for reads, `Action` for writes — the `_local` variants.**
The futures of `gloo-net` are not `Send`.
So the plain `Resource` and `Action` do not work, but `LocalResource::new` and `Action::new_local` do.

A read is a resource keyed on a version signal.
A mutation is an action that bumps that version **only on success**.
Because of that last part, an error banner can no longer be wiped by the reload that follows a failed write.
The bug that shipped in M4 came from three call sites, each of which had to remember to reload only on success.
Now it is one invariant in one place.

Error state stays **centralised**, not per action.
Each action's own `value()` is `Option<Result<..>>`.
Picking "the current problem" from several of them would need an arbitrary order of precedence, because actions carry no timestamps.
So every action and the resource write to one `problem` signal.
This keeps a single banner honest.
An action's `pending()` is still used for what it is actually good at.
It disables a control while that control's own write is in flight.
This closes the double-submit gap, which exists because `POST /projects` is not idempotent.

**Pass the signal, do not wrap it in a closure.**
Write `.disabled(action.pending())`, not `.disabled(move || action.pending().get())`.
Leptos accepts a signal, a closure returning a value, or a closure returning a signal.
All three are reactive.
But with the bare signal, the mistake cannot be written at all.
`.disabled(action.pending().get())` also compiles, and it is **silently static**.
It reads the value once while the view is built and never updates again.
This is the same trap as `.child(count.get())` versus `.child(count)`.
With a bare signal there is no closure to forget.
Switching to a one-time snapshot needs a deliberate, visible `.get()`.

A closure is still right when the value is *computed*.
For example, `move || draft.get().trim().is_empty() || workspace.renaming().get()` combines two sources, so it cannot be a bare signal.
The rule is about not wrapping a single signal in ceremony.
It is not about avoiding closures.

**The client is organized by feature, mirroring `features/`.**
There is one folder per backend domain.
Each has its own `model.rs`, `service.rs`, state holder and components.
Only truly shared plumbing sits at the top.

```
clients/web/src/
├── app.rs                  composes features
├── http.rs                 ApiError + response classification
├── projects/               model, service, workspace, overlays, components
└── scenes/               model, service, editor, the wasm-bindgen bridge
```

The alternative is grouping by *kind of thing*, and it quickly collapses under its own weight.
An `ids.rs` holding `ProjectId` and `SceneId` puts two unrelated types together, only because both are ids.
The same instinct would give us `services.rs` and `models.rs` next.
Ids live with their feature.

**`contract` stops at `service.rs`.**
A DTO may be seen by exactly one file per feature: the service that fetches it.
A state holder or a component never sees it.
Before this rule, `ProjectDTO` reached five of the ten client files, and the damage was concrete.
`row.rs` parsed RFC 3339 *inside a UI component*.
When parsing failed, it showed the raw wire timestamp to the author.
Now parsing happens once, in the service.
There, a malformed response is honestly an `Unexpected`.
On the server, the same reasoning keeps `contract` out of `core`.
Here it is applied to the other side of the wire.

**A client model must differ from its DTO, or don't create one.**
Otherwise every feature collects a renamed mirror of its wire type.
That is pure ceremony, and one more thing to keep in step.
`Project` earns its place in three ways:

- a typed `ProjectId` instead of a `String`,
- an `OffsetDateTime` instead of unparsed text,
- and no `created_at` at all, because the wire sends it and nothing renders it.

The model carries what the UI needs, not whatever the server happens to send.
`scenes` has **no** model.
A `Scene { id, text }` would only have been a mirror that does nothing.
So its service returns a bare `SceneId`.

**Ids are newtypes, and they do not validate.**
`ProjectId(String)` and `SceneId(String)` are tags, not parsers.
The point is this: in the old signature, `rename(id, name)` took two `String`s.
Now, swapping the two arguments is a compile error.
An alias (`type ProjectId = String`) would not have caught that, because an alias is documentation, not a type.
The ids deliberately do *not* check the `scene_` prefix.
[The client never computes an id](#the-scenes-feature); it sends back what it was given.
A validating constructor would quietly make the client depend on a format that the server should be free to change.

**Ids are declared, not written out.**
`ids::id!(IdeaId, "idea_")` is the whole of a feature's `id.rs`.
Four features had grown identical copies of 136 lines each.
They had the same v7 generation, the same base62 `Display`, the same prefix-stripping `FromStr`, and the same nine tests.
Every new feature was about to copy a fifth one.
The macro generates the type, its impls **and its test module**.
So the tests run once per id type, against the code that actually backs it.
They are no longer a copy that someone has to remember to keep in step.

We chose a macro over a generic `Id<Kind>`.
The generic form needs a marker type per feature, and `Idea` is already the name of the aggregate.
It needs hand-written impls to avoid the derive bounds of `PhantomData`.
And it shows up as `Id<Ideas>` in every error message.
The macro keeps the types truly distinct and named, and that is the property that matters.
A `scene_…` string must not parse as an `IdeaId`, and a generated test checks that.

The per-feature error types went away with it.
One `ids::InvalidId` carries the prefix it expected.
So the message stays exact, while four nearly identical error types disappear.
Also, `ids` re-exports `Uuid`, `OffsetDateTime` and `Duration`.
So a feature that declares an id needs no `uuid` dependency at all.
Three cores dropped theirs.

**Prefer the word the field already has.**
Invent a name only where the standard vocabulary has none.
This rule was applied late and cost a wide rename.
So it is written down, not left to taste.
The renames were:

- `absorb` became `apply`, both on the `Aggregate` trait and on `Scene`, because Yjs calls it `applyUpdate` too.
- `collapse` became `compact`.
- `Listener::hear` became `handle`.
- `Landed` became `Appended`, to match the `append` that produced it.
- `born` became `from_first`.

The invented words read well but taught the reader nothing.
Worse, one of them was actively misleading.
`Unheard` says *nobody heard this*.
It actually meant *one listener refused it while the others succeeded*.
So the name argued against the type it named.
It is now `NotHandled`.

What survives the rule are names for things the field really has no word for:

- `Recorded<E>`: an event with its metadata.
- `Standing<A>`: a state plus the version it is at.
- `Kept`/`Fleeting`.
  The RabbitMQ words *durable* and *transient* describe the queue.
  They do not describe the question a feature is actually answering: does missing this message matter?

**Name a projection for what it projects, and say that it is one.**
`IdeaCatalog` stays the name of the port.
`Catalog` distinguishes *this* read model from the board's and the outline's, which are also projections over ideas.
With `IdeasReadModel`, they would all need the distinguishing noun back.
What was missing was the fact that it is a projection at all.
So the listener's name says it: `CatalogueIdeasListener`.
That is where the CQRS rules apply, and where a reader meets them.

**A type is not prefixed with its own module's name.**
`scenes::prose::ProseEditor` already says "scene" once, through the path.
`SceneEditor` there would say it twice, and it would claim more than the type does.
The `scenes` folder holds two editor-like things.
The useful distinction between them is what each one *owns*, not which language it is written in.
`SceneEditor` is the Leptos component.
It owns the liveness indicator, the host element, the focus control, and the lifecycle that creates and destroys what sits inside it.
`ProseEditor` is that inner surface: ProseMirror plus Yjs plus the socket, reached through `wasm-bindgen`.
These names show that one contains the other, which matches how the code nests.
They also keep "prose" meaning exactly one thing at every layer: the rich-text content.
That is also the `Y.XmlFragment` key, and the `class="prose"` section that wraps it.
An editor that handled prose *and* a scene's title would need a new name.
This one does not.

**Styles are SCSS, split the way the client is split.**
`index.html` has no `<style>` block and is now eleven lines long.
The stylesheet lives in `clients/web/scss/`.
Trunk compiles it through `<link data-trunk rel="scss">`.
(*Since 2026-11-09, cargo-leptos compiles it instead, from `style-file` in the workspace's `[[workspace.metadata.leptos]]`, and `build.rs` runs the esbuild step.*)
This needs no build hook.
Trunk fetches dart-sass into its own tool cache, exactly as it already does for `wasm-bindgen`.
So the esbuild hook stays the only JS tooling in the client.
The partials mirror the source tree.
`_tokens`, `_base` and `_controls` are the shared vocabulary.
`_projects`, `_overlays` and `_prose` follow their features.
The compiled CSS lands in `dist/`, outside the watch root.
So, unlike the JS bundle, it carries no risk of a rebuild loop.

Two rules are not obvious from the files:

- **The palette stays CSS custom properties, not Sass variables.**
  `--ink` and its siblings are reassigned at runtime by the `prefers-color-scheme` media query.
  A Sass variable is substituted at compile time and cannot do that.
  Here Sass gives us nesting and file structure, and nothing for the color system.
- **Third-party class names stay flat and literal.**
  `.ProseMirror`, `.ProseMirror-hideselection` and `.ProseMirror-yjs-cursor` could be folded together with `&-` interpolation.
  But then you could not grep for them in the library that defines them.
  Nesting is for our own descendants (`.dialog h2`, `.surface blockquote`).
  It is never used to build a name.

The port to SCSS was verified, not just checked by eye.
There were 163 declarations before and 163 after.
The only difference was that dart-sass removed the quotes in `[contenteditable="false"]`.

**Routes are paths, and `leptos_router` owns them.**
`/` is the workspace, and `/projects/{id}` is one project's pool.
`route.rs` now contains only the path builders, so one place knows the URL shapes.
The route table itself is a `view!` block.
We use paths and not query parameters, because paths *nest*.
`?project=x&scene=y` says nothing about how the two relate.
`/projects/{id}/ideas/{id}` states it.
That is the shape the outline and the board will need.

**Navigation is `<A>`, never a button with a callback.**
At first this was written by hand: a `Route` enum, a parser, and `pushState` plus a `popstate` listener.
It was wrong for a reason that had nothing to do with how many routes there are.
Navigation was a `<button on:click>`.
So a project could not be ctrl-clicked into a new tab.
It showed no URL on hover.
It offered no *copy link address*.
And a screen reader announced it as a button.
That defeated the whole point of moving to paths, which was shareable links.
`leptos_router`'s `<A>` renders a real anchor and intercepts only unmodified left-clicks.
So the browser's own features keep working.
The router also handles `popstate`, nested routes, and route matching.
That is the pile of work that writing it by hand was quietly signing up for.

The earlier reasoning about `replaceState` still stands where it is written.
For routes, this replaces it: with real routes, the answer is to handle history properly, not to avoid history entries.
**Ids stay in the path, never slugs.**
A slug is derived and can change, and project names are neither unique nor stable.
See [TODO.md](./TODO.md) for the decorative-slug pattern.
It gives readable URLs without turning a name into an identity.

**The open scene lives in the URL.**
`?scene=<id>` is all of that state.
A reload reopens the same document.
A second tab pointed at the same address joins that document, not a copy of it.
This is exactly what makes the collaboration testable at all.
It is written with `history.replaceState`, not `pushState`.
Nothing listens for `popstate` yet.
So a Back entry would rewind the address bar but leave the editor open.
That would be a lie about where you are.

Sometimes the address names a scene that the server does not know.
Then the client drops the parameter and offers a fresh start, instead of mounting an editor.
This is not about tidiness.
The socket would never connect.
But ProseMirror would still render an editable surface that seems to work.
So every word typed into it would be lost.
The client does not inspect the id to decide this, because [it does not validate ids](#client-conventions).
It asks the server, which is the only authority on what exists.

**Client tests are end-to-end, in a real browser.**
We use Playwright against the real API.
Since 2026-11-09, the API also serves the client, and it is started by `cargo leptos serve`.
See `clients/web/e2e`.
The alternatives were weighed and rejected based on evidence.
Both client bugs that actually shipped in M4 were *browser behavior*:

- a `<form>` that submitted natively and reset every signal,
- and an error banner that was cleared by an unconditional reload.

Neither a unit test behind an API port nor a mounted-component test would have caught either of them.
Clicking through Chrome did.
Using `LocalResource`/`Action` also shrinks what can be unit-tested, because what remains in the client is declarative wiring.

The selector discipline keeps such tests from rotting: **role plus accessible name, never a CSS class, an index or XPath**.
Restyling cannot break a test.
Breaking `aria-label="More actions"` can break one, and it should, because a screen-reader user just lost that button.
The accessibility work done in M4 turned out to also serve as the test surface.
Playwright's auto-retrying locators remove the other classic source of flaky tests.
So the suite contains no sleeps.

Two traps were found while setting it up, and both are worth remembering.

- **Playwright must write its output outside the Trunk watch root.**
  `outputDir` points at `target/`.
  Also, `e2e/`, `node_modules/` and the JS manifests must be in `[watch] ignore`.
  Otherwise a test run triggers a rebuild, the page reloads during the test, and elements detach under the cursor.
  That caused a flake in one of five runs, in a different test each time, and 257 needless rebuilds.
  It is the same kind of mistake as the spike's rebuild loop.
- **`hasText` matches substrings.**
  So a new name in a rename test must not contain the original name.
  Otherwise the "old name is gone" assertion silently passes against the new row.

### What the client changes, and what it is told — decided

*Written on 2026-11-05.*
*The user asked about it after a held arrow key made a card snap back.*
*It was settled with the user on the same day:*
*it is called a `WorkingCopy`, every aggregate gets one, and every change carries `If-Match`.*
*Not built yet.*
*It replaces the hand-written handling that each view grew on its own.*

Three streams meet in the client.
Every view that changes something has to bring them together:

1. **What the author does**, often fast.
   For example, a card nudged by a held arrow key, a section moved up three times, or a title being typed.
2. **What the client asks the server**: the requests that those actions turn into.
3. **What the client receives**: the answers to its own requests, and the changes of other authors over the live socket.
   Both carry the aggregate's version.

**What is shown is the confirmed state with the pending changes laid over it.**
For each aggregate, the client keeps the last state that the server confirmed, together with its version.
On top of that, it keeps its own changes that are still on their way.
Anything that arrives goes into the confirmed state by version.
That can be the answer to its own request, or someone else's change.
Older or repeated versions change nothing.
Then the pending changes are laid over the confirmed state again.
So an answer is nothing special: it is just one more version.
This is how Figma and Linear keep their screens both immediate and correct.
The board already has half of it: `confirmed` with its version, and `Intentions` with their tickets.
The outline has the same, but only for retitles.

**One request at a time per aggregate, in the order they were made.**
Requests sent side by side reach the server in any order.
Their answers also come back in any order.
That alone causes the snapping back.
So each aggregate sends one request, and sends the next one only after the first is answered.
The next request is worked out at that moment, from what is shown then.

**Merged, not debounced.**
A debounce waits a fixed time before sending.
That delays every single key press.
And it loses a change if the tab is closed during the wait.
Merging does the same work without the wait:

- When nothing is on its way, a change goes out at once.
- While a request is on its way, the next changes collect.
- Of those, only the newest of each kind goes out.

A held arrow key sends the first nudge at once.
Then it sends one more, with the place where the card ended up.

Changes that set something outright may be merged.
Examples are a card's spot and size, a title, or a role.
The newest one wins, keyed by what it is about.
Changes that build on the step before cannot be merged.
Examples are moving a section up by one, or promoting it.
Such a change is sent in its turn.
Better still, it is turned into an outright change when it is sent, worked out from what is shown at that moment.
For example: "place it after the section that is now above it".

**Every change says what it was made on: `If-Match`, always.**
The server accepts `If-Match` on every aggregate.
A command without it is decided against whatever state the server holds (`serving::demanded`).
Until now, the client sent it only for a project's details.
With one request at a time, the client always knows the version it builds on.
So every change sends it.
If the server still holds that version, the change lands.
If the server holds a newer version, because another author got in between, it refuses the change with 412.
The working copy then takes in the newer state, lays its pending changes over it again, and sends the change worked out anew.

The outline shows why this matters:

- The author sees A, B, C and moves C up.
  This goes out as "C after A".
- Meanwhile, another author moved A to the end.
  So the server holds B, C, A.
- Without `If-Match`, "C after A" gives B, A, C.
  C went down, although the author moved it up.
- With `If-Match`, the change is refused instead.
  It is worked out again from B, C, A, as "C before B".

An outright change, such as a card's spot, comes out the same the second time.
For such a change, a refusal costs one round trip, and only when someone really did get in between.
Having one rule for every change is worth that cost.

**Where it applies: every aggregate.**
All aggregates have used it since 2026-11-05.
Nothing is treated as simple until it proves to be simple.
If an aggregate turns out not to need it, that can be decided again then.

| Aggregate | Changes made fast or in a row | Merged by | Today |
|---|---|---|---|
| Board | reshape (spot, size), pin, unpin | card | `WorkingCopy<Board>` |
| Outline, in the outline view | retitle; move up and down, promote, demote, place; add, remove; attach, detach | section, for retitles; the rest in turn | `WorkingCopy<Outline>` |
| Outline, in the writing view | retitle, nest, lift, attach, detach, add, remove | section, for retitles | `WorkingCopy` of its own over the same aggregate |
| Ideas | retitle | idea | `WorkingCopies<IdeaId, Idea>`, the board and its inspector sharing one |
| Scenes | retitle, link and unlink ideas, delete | scene, for titles | `WorkingCopies<SceneId, Scene>` in the writing view |
| Projects | rename, details, trash, restore | project | `WorkingCopies` for the list and the trash; the open project has its own copy, because its details are not in the list |
| Members | change of role, removal, leaving | member | `WorkingCopy<Members>`, the version in `ETag` |
| Invitations | withdraw; for invitations to oneself, accept and decline | invitation | `WorkingCopies`; accepting and declining carry no `If-Match`, because they act on one's own account with a single-use ticket |
| Notices | dismiss | notice | `WorkingCopies` |
| Accounts, the operator's list | lock, unlock, appoint, dismiss | account | `WorkingCopies` |

So the accounts, invitations, notifications and the rest also go through a working copy.
This includes the actions that happen only once: creating, trashing, inviting, answering.
Each of them is a change like any other, and the working copy sends it in its turn.
Only a scene's prose is left out.
It is not an aggregate but a CRDT, and its own socket keeps it correct.
Today some places use an `Act` or an `Asking`.
As each aggregate moves over, the working copy takes their place.
They are removed once nothing uses them.

**A list of aggregates is `WorkingCopies`.**
Examples are the ideas of a project, the projects of an account, a project's invitations, and a user's notices.
Each item is an aggregate with its own version.
So each item gets its own `WorkingCopy`, under one `WorkingCopies<Id, A, I>`.
A change to one item never waits for another item.
The list, as read from the server, goes into `take_in_all`.
A single item received from the server goes into `take_in`.
An item that is gone goes into `forget`.
If an item is removed while one of its intents is still on its way, that intent is reported as not landed.
Only the board, an outline and a project's members are a single aggregate each.

**Built once, used by each: the `WorkingCopy`.**
`WorkingCopy<Board>` and `WorkingCopy<Outline>` live in `clients/web/src/working_copy.rs`.
A working copy holds:

- an aggregate's confirmed state,
- its pending `Intent`s,
- the one request that is on its way,
- and the merging.

It answers a 412 by laying the pending changes over the newer state.
The name comes from version control.
It is what the author works on: the confirmed state with their own changes on top, matched with everyone else's changes as they go out.
A pending change is an `Intent`, such as `boards::Intent` or `outline::Intent`.
It is what the author means to do, and the server has not confirmed it yet.
Its name keeps it apart from a change that the server reports, such as `OutlineChange`.
That one is confirmed and carries a version.
Each view states:

- what its changes are,
- how a change is laid over the state,
- which changes merge,
- and how an answer and a reported change are taken in.

A view no longer writes any of this by hand.
It is to be built one aggregate at a time.
The board comes first, because the bug shows there.
The outline comes next, because moving several times in a row needs it most.
Then the rest follow.
Each gets a browser test that holds a key or moves several times in a row.

## Export

This is **manuscript export**: turning a project into something a reader can read.
It is unrelated to the **project export** in [Two Ways to Run It](#two-ways-to-run-it).
That one turns a project into a file its author can carry around.

There are two separate export pipelines.
Both are driven by an in-order walk of the structure:

- **PDF** via **Typst**.
  Typst is Rust-native typesetting, embedded as a library.
  We implement its `World` trait to resolve fonts and files.
  The output is print quality.
- **EPUB** via a separate path: structure walk → semantic HTML → EPUB package (reflowable XHTML+CSS).
  It reuses the web view's HTML rendering.
  Typst does **not** produce EPUB.

Font bundling/licensing for embedded fonts is a concern for both.

## Supporting Concerns (noted for later)

- **Search & mention detection** — projections built with **Tantivy**, which is Rust-native and similar to Lucene.
  They give full-text search over prose.
  They also index codex aliases and mentions, for back-links like "which scenes does this character appear in".
- **Blobs** (codex and research images) — stored in object storage or on the filesystem, and referenced by ID from events.
  They are not stored as byte columns, and not in the event log.
- **Multi-author** — this implies accounts, project membership, and permissions.
  It is real scope, and we acknowledged it early.
- **Tenancy / ownership scoping** — today every store is **global**.
  `ProjectStore::list()` returns *every* project.
  `get(id)` returns any project to anyone who names its id.
  That is correct only while there is exactly one user.
  This is true for the MVP, and false as soon as accounts exist.
  To be clear: this is **not a filter to add on later**.
  Scoping changes the port itself (`list(owner)` instead of `list()`).
  And every method that takes an id gets an ownership check.
  Related: **ids are not capabilities**.
  A v7 id cannot be guessed in practice, because it has 74 random bits.
  But it reveals its creation time, and it will end up in logs, URLs and shared links.
  So authorisation must be explicit.
  It must never be implied by someone knowing an id.
- **Idempotent requests** — wanted eventually, and deliberately deferred.
  Only *creation* needs it.
  `POST /projects` is not idempotent.
  So a flaky network, a double-click or an automatic client retry creates a duplicate project.
  `PATCH /projects/{id}` is already idempotent.
  Applying the same rename twice gives the same state, except for a cosmetic `updated_at` bump.
  So it needs nothing.
  When we do add it, the key belongs in an **`Idempotency-Key` header, not in the request DTO**.
  The key is a delivery concern, not part of the resource representation.
  Putting it in `contract` would force the concept on every future adapter.
  It also needs real infrastructure: storing key → response on the server with a TTL, which means a new port.
  That would be hollow with an in-memory store that loses the keys on restart.
  A cheaper alternative exists.
  The client could generate the `ProjectId` and use `PUT /projects/{id}`.
  That makes creation naturally idempotent, with no key store at all.
  But it reverses the decision that *the server owns id generation*.
  That decision is why `contract` has no `uuid` dependency, and why ids travel as strings.

## Status

Phases 1 and 2 are done, and Phase 3 is being built. Where it stands is in [ROADMAP.md](./ROADMAP.md).
