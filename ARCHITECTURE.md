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

- **`core`** — domain types, business logic, the **ports** (traits) it needs, and the application facade (`ProjectService`). Plain Rust, no frameworks, no magic. **Depends on no other crate in its own feature** — in particular not on `contract`, so the wire format can never leak inward. Cross-cutting library ports are the one exception (see [cross-cutting ports](#dependency-rules)); `libraries/clock` is why `core` is a near-leaf rather than a strict one.
- **`adapters/*`** — **one crate per adapter**, named for the foreign system it talks to. All arrows point inward at `core`.
  - *Outbound (driven)* adapters implement a port declared by core. Named after the port: `ProjectStore` → `store`. A port with several **interchangeable backends** stays **one** crate — `memory` and later `postgres` are modules inside `store`, not sibling crates. They are backends of one adapter, not two adapters. Heavy optional backends go behind a cargo feature (`postgres = ["dep:sqlx"]` with `sqlx` marked `optional`), so a build that never enables it never compiles it. Two consequences: the swap is one feature flag rather than one dependency line, and **CI must run `--all-features`**, because feature-gated code is not type-checked otherwise.
  - *Inbound (driving)* adapters call core's public API. Named after the transport: `rest`, later `graphql`, `cli`, `messaging`. No inbound port trait — `ProjectService` is already the interface.
- **`contract`** — the wire types. Deliberately **not** part of the onion: it is a shared kernel between two *processes*, and it exists as its own crate for a hard technical reason — the WASM client cannot depend on `rest`, because `axum` doesn't compile to WASM. The constraint is **`serde` and nothing else**. Values travel in their primitive wire representations — ids and timestamps as strings — while the rich domain types (`ProjectId`, time types) stay in `core` and the adapter maps between them. This keeps the crate trivially WASM-safe and gives the domain vocabulary exactly one owner. Note that ids arriving *inbound* come through the URL path, which `rest` parses with its own extractor, so contract is almost entirely an outbound-shape concern.

**A constant both processes must agree on lives in *both*, pinned by a test.** The fragment name a passage's prose lives under (`"prose"`) is needed by `core` to project plain text and by the client to write into the same document — and `core` must not reach into `contract` to get it. So each defines its own, and the feature's tests crate, which can see both, asserts they match:

```rust
assert_eq!(passages_core::FRAGMENT, passages_contract::FRAGMENT);
```

A duplicated literal justified by a test is normally a smell. It wins here because the alternative is worse: one `core → contract` arrow "just for a constant" is the crack DTOs leak through later, and the invariant is only worth having while it is absolute. The failure this test prevents is a silent one — mismatched fragment names mean prose that vanishes with no error anywhere.

There is deliberately **no `boundary` crate**. Once every adapter has its own crate, "boundary" names whatever is left over — which is nothing. Mapping belongs to the adapter that owns the DTOs, the facade lives in core, and wiring is the service's job.

**When *not* to split an adapter out:** the test is *different dependencies, or a different swap story?* REST vs. store: yes to both — split. REST vs. a hypothetical GraphQL adapter over identical DTOs: no to both — one crate. Two backends of the same port are never split: they answer "no" to the swap question by definition, since they are alternatives to each other rather than things that coexist. Split on dependency weight and swappability, not on conceptual tidiness.

**Feature-level tests live in their own crate.** `features/<name>/tests` is **test-only**: an empty library plus `#[cfg(test)]` modules, depending on every crate in the feature and depended on by nothing. It exists because cross-adapter tests have nowhere else legal to live — putting them in `adapters/rest` would need a `rest → store` dev-dependency (the forbidden sibling arrow), and putting them in `core` would need `core → adapters/store`, inverting the onion. A leaf that depends on the whole feature keeps every arrow pointing inward.

The payoff is that feature tests use the **real** adapters. A hand-written fake store is a second, unverified implementation of the port — it cannot run against the conformance suite, so it is free to drift from the contract and let tests pass against behaviour no real backend exhibits. Using `InMemoryProjectStore` removes that hazard entirely. It is deliberately **not** a wiring crate: no composition happens here, because that is still the service's job. Give it a test-only crate's dependency list — everything under `[dev-dependencies]`, nothing under `[dependencies]`.

There are four test scopes, and keeping them apart matters:

| Scope | Asks | Lives in |
|---|---|---|
| **Unit** | is this type's logic right? (`ProjectName` validation, id ordering) | the crate that owns the code |
| **Port conformance** | does every backend honour what the port promises? | `adapters/<port>/src/suite.rs` |
| **Feature behaviour** | does the feature behave correctly through its facade and router, with real adapters? | `features/<name>/tests` |
| **App wiring** | is the feature mounted at the right path with the right middleware? | `services/api/tests` — smoke tests only |

Feature tests are written **from a business perspective**: they assert observable outcomes, never collaborations. "The store was not called" is a technical detail invisible to any client, and asserting it couples the test to the current call order. Spy on a collaborator only when the interaction *is* the requirement — don't charge a card twice, don't send two emails — which a read from our own store never is.

**Port conformance is tested once per port, inside the adapter crate.** Because a port's backends share one crate, the suite that defines what the port *means* — `create` conflicts on a duplicate id, `update` is `NotFound` on a missing one, `list` returns creation order — lives in a plain `#[cfg(test)]` module there (`store/src/suite.rs`), taking `&impl ProjectStore`. Every backend runs the same cases; backend-specific behaviour (transaction rollback, connection failures mapping to `StoreError::Backend`) goes in that backend's own tests. `core` stays free of test scaffolding.

**Adapters are siblings, never friends.** `rest` must not depend on `store`. Choosing the concrete persistence implementation is the **service's** job as composition root — that is what keeps the in-memory → PostgreSQL swap down to one line in one manifest.

### Dependency rules

| Layer | May depend on | Never |
|---|---|---|
| `clients` | contract crates | features, services |
| `services` | features, libraries | clients |
| `features` | libraries, own contract (adapters only — never `core`) | **other features** |
| `libraries` | libraries (shallowly) | features, services |

The **feature → feature** ban is the load-bearing one. When two features want the same thing there are exactly two legal moves: the *service* composes them, or the shared concept sinks into a `libraries/` crate. (Expected first case: structure tree and timeline sharing a data structure.)

**Cross-cutting ports live in `libraries/`, not in a feature's `core`.** The rule that a port is declared by the core needing it holds for domain-specific ports like `ProjectStore`. A port that is *domain-free* and wanted by every feature goes in a library instead — `libraries/clock` is the first, holding the `Clock` trait plus `SystemClock` and `FixedClock`. The reason is stronger than anticipated reuse: per-feature copies of `Clock` would be **incompatible types**, so the composition root would have to build one clock per feature and no test could pin time across the system. This is legal under the table above (`features` → `libraries`) and is an explicit *exception* to port ownership, not a violation of it.

Note that `FixedClock` is plain `pub`, not `#[cfg(test)]` — test scaffolding in a library crate must be, since `cfg(test)` does not cross crate boundaries. That is fine here because a fixed clock is legitimately useful outside tests (replay, backfill).

### Naming and conventions

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

The editor, the shared type and the projection are settled in [Prose — the editing stack](#prose--the-editing-stack).

### Presence is neither

Cursor position, selection, and who's-online are **ephemeral awareness** state. They are **never** event-sourced and **never** persisted — they travel over the WebSocket (Yjs awareness protocol) and are discarded.

## Prose — the editing stack

Unlike the rest of this document, these decisions are **backed by running spikes** (`spikes/crdt`, `spikes/sync`, `spikes/editor`) rather than by reasoning alone. Every claim below that says *proven* is a test or a verified browser session.

**Editor: ProseMirror.** Chosen over Quill for **decorations** — a presentation layer painted *over* the document rather than into it. Spellcheck squiggles, codex mention highlights, thread anchors and later "who wrote this" are all views, not content: they must not end up in the CRDT, in the export, or in another author's replica. Quill's Delta model has no equivalent — highlighting a word means writing a format attribute into the content. ProseMirror also brings a real schema, which is what makes "an author may put a table here" a checkable statement. Licensing: ProseMirror, `y-prosemirror` and Yjs are all MIT, so a proprietary product is fine.

**Shared type: `Y.XmlFragment`, not `Y.Text`.** A node holds whatever the author put in it. `Y.Text` models one flat run of characters with marks and cannot express block structure at all. `y-prosemirror` maps a ProseMirror document onto an `XmlFragment` of `XmlElement`s and `XmlText`s — that is the thing we store, sync and persist.

**Node granularity belongs to the author.** A structure-tree node is *not* defined to be a paragraph. One node per chapter is legal; one per paragraph is the likelier default; nothing in the system may assume either. The consequence is that **merge granularity follows node granularity**: two authors inside one node share a CRDT and contend, two authors in sibling nodes never touch. Coarser nodes mean more contention — an author's trade-off, not a constraint we impose.

**One CRDT document per node.** Not one per book. A book-sized document means every reader loads the entire history and every keystroke touches one hot object. Per node keeps updates small, allows lazy loading during an in-order walk, and makes compaction a per-node job that can run incrementally.

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

**Wire compatibility is the point.** Speaking the standard format means the client can use `y-websocket` as-is and we inherit reconnect-with-backoff, resync, and awareness timeouts rather than writing them. It also decouples the halves: either side can be replaced independently, which is exactly the optionality the [Frontend](#frontend--full-stack-rust-provisional) escape hatch depends on.

**The server is a participant, not a relay.** Each room holds its own `Doc` and applies every update. That is what lets a peer who was offline for a week be served by the room rather than by whichever other client happens to be connected, and it is the hook that persistence, compaction and search projections will attach to. **A room is a node's document** — one `Doc` per room lines up with one CRDT document per node.

**Awareness is relayed, never decoded.** The server treats awareness frames as opaque bytes and forwards them; it holds no presence state at all. Late joiners still get cursors, because on join the server broadcasts *query-awareness* and the connected peers republish themselves — an ordinary y-websocket client answers that automatically. This keeps presence genuinely ephemeral, exactly as [Presence is neither](#presence-is-neither) requires.

The known cost of that choice: **the server cannot retract a departed peer's cursor**, because it does not know which client ids a connection spoke for. Stale cursors linger until the client-side timeout (~30 s). The fix is to decode just the awareness header (client id, clock, state) and broadcast a tombstone on disconnect — small, and deliberately deferred.

Two things to revisit before this is production code: an unreadable frame is currently **logged and skipped**, which means a peer sending an unusable update diverges silently rather than being disconnected; and rooms live in memory for the process lifetime, with no persistence, no eviction, no backpressure and no auth on the socket.

### The `passages` feature

The spikes proved the technology. This is where it lands in the codebase.

**The feature is `passages`; the aggregate is `Passage`.** It mirrors `projects` / `Project` exactly, so `PassageId`, `PassageStore` and `PassageService` all follow without anyone having to think. `text` was ruled out on collision grounds — `yrs::Text`, `Y.Text` and ProseMirror's text nodes all already exist here, and a `Text` aggregate beside them is a permanent "which one?". `prose` was the runner-up and survives as *vocabulary* (and as the CRDT root key), but it is a mass noun: it does not pluralise like every other feature, and `Prose` makes an awkward type. The word an author would recognise and the word that fits the codebase turned out not to be the same word, and the aggregate name is for us.

**A passage is its own aggregate because it has its own consistency model.** Structure is event-sourced with optimistic concurrency on a version column; a passage is a CRDT that merges without coordination. One cannot be nested inside the other without one of them being wrong. The feature split is not tidiness — it is the two-speed model made structural.

**A passage has its own id; the node links to it.** `PassageId` is a UUID v7 like every other identifier here — generated when the passage is created, derivable from nothing. Which passages a node shows is carried by the node's **event stream**, as `PassageAttached { passage_id }`.

*Considered and rejected: making a passage's id equal its node's id.* It is genuinely tempting — the passage becomes derivable, so bringing one into existence needs no coordinating write at all. Two things sank it:

- **It bakes "one passage per node" into the key.** A node plausibly wants a body, a synopsis and author's notes (the Scrivener arrangement), or two drafts of the same scene. Any of those needs a discriminator, and we would have to guess its shape *now* — is the second axis a **role** or a **variant**? Guess wrong and the key gets reshaped twice. With an opaque id the semantics live in the link instead, and a link is far cheaper to extend than a key.
- **The risk it avoided was smaller than it looked.** The worry was a two-phase create across two aggregates with no shared transaction. But **ordering** fixes that, not identity: write the passage first, emit the attachment second. A failure between them leaves an **orphan** — unreferenced bytes a sweep collects — not a **dangle**, which is a reference to something that does not exist. Orphans are benign; this is ordinary referential discipline rather than a new class of bug.

What independent ids buy is worth more than the one write they cost: prose can **move between nodes carrying its CRDT history**, which is what makes the open split-node question answerable at all; a passage could exist without a node if free-floating research notes ever want one; and `PassageId` is a plain v7 newtype exactly like `ProjectId`, with no special rule to remember.

**The link lives on the node, not on the passage.** "Which writing this node shows" is a fact about the book's *shape*, so it belongs in the structural history where undo and the audit log can see it. It also keeps the arrows clean: `structure` holds an opaque id string and never names a passage type, while `passages` never learns what a node is. Storing an `owner: NodeId` on the passage instead would drag structure's vocabulary into the passages core and leave attachment out of the audit log.

This is also what makes multiple passages per node cheap when we want them: the discriminator arrives as an **event field** — `PassageAttached { passage_id, role }` — which is the cheapest place in the whole design to add one, given that event versioning and upcasting were planned from day one. No key reshape, no id migration.

- **A node that holds no writing has no passage.** Nothing is allocated until a key is pressed, so grouping nodes ("Part One") cost nothing.
- **The client never computes a passage id — it asks.** The derivation, the ordering, the link: all of it stays server-side. This is what keeps the scheme changeable without touching a single client.

Cross-aggregate cleanup runs the other way: deleting a node must eventually dispose its passages, across two aggregates with no shared transaction. That is the **first real job for the transactional outbox** — eventually consistent by construction rather than by concession — and it is the same sweep that collects orphans.

**Crate layout**, following the [feature anatomy](#feature-anatomy--the-onion):

```
features/passages/
├── contract/          nearly empty — the wire format is y-protocols, not serde
├── core/              Passage, the plain-text projection, the PassageStore port
├── adapters/
│   ├── store/         in-memory now, PostgreSQL later
│   └── sync/          the y-protocols codec, socket plumbing, the room registry
└── tests/
```

Three placements that are not obvious:

**`yrs` belongs in `core`.** This reads like a violation of *core has no frameworks*, and it isn't: the CRDT **is** the domain model of a passage, not an implementation of it. Swap `yrs` for Automerge and the merge semantics change — that is substance, not a swappable detail. The dividing line that matters: `axum` is transport, `sqlx` is persistence, `yrs` is the thing itself.

**The wire protocol stays in `adapters/sync`.** Core exposes intent — *catch up from this state vector*, *apply this update* — and the adapter does the message framing. Same rule as `rest` mapping DTOs with free functions, and for the same reason: a wire format has no business in the domain.

**Core owns a room's semantics; the adapter owns the registry.** The sync spike found this seam without being asked. `room.rs` is a pure function from a message to a reaction, with no tokio, no axum and no sockets — that is core material. `server.rs` is connection bookkeeping, broadcast channels and lifecycle — that is adapter material. Keeping them apart means a second transport could reuse the semantics unchanged.

**Awareness never reaches `core` at all.** It is relayed as opaque bytes and it lives and dies inside `adapters/sync`. Presence is ephemeral by decision; this is that decision expressed as a dependency arrow.

## Local-First

Because prose is a CRDT, the client keeps a **full replica in the browser (IndexedDB)**. This gives us, almost for free:

- **offline editing** — the server being down or unreachable never blocks writing,
- **zero typing latency** — edits apply locally and sync in the background,
- a concrete expression of the "your data is yours" value.

The client is designed to be **local-first-capable**, not a thin dumb terminal.

## Transactions and Atomicity

> **Atomicity is a property of a single port method.** If two things must happen atomically, make them **one method** — never introduce a transaction object into a port.

Coarser ports are the price, and it is cheap. Two decisions already made are what let us get away with it.

**The aggregate is the transaction boundary.** The orthodox DDD answer, and the right one. An event store port is one method:

```rust
async fn append(&self, stream: StreamId, expected: Version, events: &[Event]) -> Result<(), StoreError>;
```

The version check and the append are atomic *inside* it — PostgreSQL uses a real transaction, the in-memory backend holds its write lock, and a version mismatch is a `Conflict`. The caller never learns which mechanism was used, because it is none of the caller's business.

**The outbox is an adapter detail.** This is where the reach for transactions usually happens, so it is worth being blunt: the port says *append these events*. The PostgreSQL adapter appends them **and** writes the outbox rows in one transaction. An in-memory adapter appends them and pushes onto a queue. Both are atomic in their own world; neither leaks its mechanism upward. Nothing above the port knows an outbox exists.

Everything in this design fits the rule already. Cross-aggregate work — deleting a node disposing its passages — was *chosen* to be eventually consistent through the outbox, and cross-feature work is banned outright.

### No `Tx` in a port signature

The tempting alternative, `async fn create(&self, tx: &mut Tx, project: Project)`, costs three things:

- It puts **persistence vocabulary into a port that `core` declares** — precisely the leak the onion exists to prevent.
- Rust has **no ambient transaction context**. There is no `@Transactional` and no thread-local that survives `.await` cleanly, so `Tx` has to be threaded through every signature by hand, forever.
- The in-memory backend would have to **fake** a concept it does not have. A no-op `Tx` is worse than none: it makes tests pass that would fail against a real database.

The closure form (`uow.run(|ports| async { … })`) is conceptually nicer and fights async Rust hard — higher-ranked lifetimes, borrows across `.await`, dyn-compatibility. Not worth it for a problem the aggregate boundary already solves.

### How an in-memory backend is atomic

Take the lock **once**, validate, then mutate, all under the same guard:

```rust
let mut projects = self.write();
if projects.contains_key(&project.id()) {
    return Err(StoreError::Conflict(project.id()));
}
projects.insert(project.id(), project);
```

No caller can observe a half-state, and an in-memory operation has no I/O to fail partway through. One discipline makes it hold: **never hold a `std::sync::RwLock` guard across an `.await`** — it blocks the executor and the guard is not `Send`.

**What in-memory can prove:** observable atomicity — a failed `create` leaves the store unchanged, a version conflict appends nothing. Those belong in the **conformance suite**, so every backend must honour them.

**What it cannot prove:** that a real backend actually wrapped its statements in a transaction. In-memory tests pass happily while PostgreSQL runs two unwrapped statements. There is no clever fix; the mitigation is that such tests live with the backend, which is already [how the test scopes are split](#repository-structure).

### A worked example: `PassageStore::absorb`

`absorb(id, update)` takes a delta and lets the backend choose its representation — and the two choices have genuinely different concurrency needs, entirely inside the method:

- **Snapshot backend:** read bytes, merge the update, write bytes. Two concurrent absorbs both read `S`; one writes `S+A`, the other `S+B`, and **one is lost**. CRDTs do not save you — the loss happens below the merge. This backend needs `SELECT … FOR UPDATE` or a version column.
- **Append-only log backend:** one insert. Appends commute, so concurrent absorbs need no locking at all.

Today this is masked because the room owns the document, so there is a single writer per passage per process. That assumption dies the moment a second server instance exists. Either way it stays *inside* `absorb`, which is the rule working exactly as intended.

## Event Sourcing — Discipline

- **Be selective.** Event-source only where history has value (the structural domain, undo/redo). Plain state for settings, presence, search indexes, and blobs. "ES where history has value" — not everything.
- **Event store:** rolled on **PostgreSQL** — an events table with per-aggregate streams and optimistic concurrency via a version column.
- **Projections / async work:** a **transactional outbox** + poller for reliable projection updates; PostgreSQL `LISTEN/NOTIFY` (or logical replication) to nudge the WebSocket gateway. No message broker for now (see below). The outbox lives *inside* the event store adapter and is invisible above the port — see [Transactions and Atomicity](#transactions-and-atomicity).
- **Versioning / upcasting** is planned from day one — a book project lives for years and events will evolve.
- **Snapshots** for aggregates to keep rebuilds fast at scale (a paragraph-per-node book is thousands of nodes).

## Messaging Broker — Deferred (option kept open)

We start **without** a broker (no RabbitMQ/AMQP yet). Inside a modular monolith, in-process dispatch + the Postgres-backed outbox goes a long way and avoids dual-write/ordering complexity before it pays off.

A broker can be introduced later at a real module boundary — e.g. when there are separately-deployed consumers or cross-service async handoff — **without rearchitecting**, provided the module seams stay clean. Keeping this option open is an explicit goal.

## Frontend — Full-Stack Rust

We take the **full-stack Rust** path (Leptos, compiled to WASM). It's part of the fun and keeps the whole stack in one language.

This was provisional until the editor spike settled it. The worry was that a collaborative rich-text editor would be too heavy in Rust/WASM, and the fallback was to switch the frontend to **Angular** with a proven editor plus Yjs bindings, keeping the Rust backend. That escape hatch stays open and costs nothing to keep open — the interop boundary is bytes and the sync protocol is the standard one, so neither half knows what the other is written in. But nothing found so far argues for taking it: hosting ProseMirror from Leptos is a ~20-line boundary, and the bundle cost is ordinary. See [The client hosts the editor](#the-client-hosts-the-editor-rust-never-touches-the-document-model).

## Export

Two distinct export pipelines, driven by an in-order walk of the structure:

- **PDF** via **Typst** (Rust-native typesetting, embedded as a library — implement its `World` trait for font/file resolution). Print-quality output.
- **EPUB** via a separate path: structure walk → semantic HTML → EPUB package (reflowable XHTML+CSS). Reuses the web view's HTML rendering. Typst does **not** produce EPUB.

Font bundling/licensing for embedded fonts is a concern for both.

## Supporting Concerns (noted for later)

- **Search & mention detection** — projections built with **Tantivy** (Rust-native, Lucene-like): full-text search over prose, plus codex alias/mention indexing (the "which nodes does this character appear in" back-links).
- **Blobs** (codex and research images) — object storage or filesystem, referenced by ID from events. Not stored as byte columns and not in the event log.
- **Multi-author** — implies accounts, project membership, and permissions. Real scope, acknowledged early.
- **Tenancy / ownership scoping** — today every store is **global**: `ProjectStore::list()` returns *every* project, and `get(id)` will hand back any project to anyone who names it. That is correct only while there is exactly one user, which is true for the MVP and false the moment accounts exist. Worth being clear that this is **not a filter to bolt on later**: scoping changes the port itself (`list(owner)` rather than `list()`), and every id-taking method gains an ownership check. Related: **ids are not capabilities**. A v7 id is unguessable in practice (74 random bits), but it leaks its creation time and will end up in logs, URLs and shared links — so authorisation must be explicit, never implied by someone knowing an id.
- **Idempotent requests** — wanted eventually, deliberately deferred. Only *creation* needs it: `POST /projects` is not idempotent, so a flaky network, a double-click or a client auto-retry produces a duplicate project. `PATCH /projects/{id}` already is idempotent — applying the same rename twice yields the same state, bar a cosmetic `updated_at` bump — so it needs nothing. When we do add it: the key belongs in an **`Idempotency-Key` header, not in the request DTO**, because it is a delivery concern rather than part of the resource representation, and burying it in `contract` would force the concept on every future adapter. It also requires real infrastructure — server-side storage of key → response with a TTL, i.e. a new port — which would be hollow against an in-memory store that loses it on restart. A cheaper alternative exists: let the client generate the `ProjectId` and use `PUT /projects/{id}`, making creation naturally idempotent with no key store at all — but that reverses the *server owns id generation* decision, which is why `contract` carries no `uuid` dependency and ids travel as strings.

## Status

Phase 1 is built — the projects slice runs end to end in a browser (see [ROADMAP.md](./ROADMAP.md)). The structure domain and event sourcing are still design. The prose stack is spiked, measured and proven in all four of its risky places — CRDT semantics, `yrs` ↔ `Yjs` compatibility, the editor in Leptos, and sync over WebSocket — but none of it is wired into the application yet. The solution is still being woven.
