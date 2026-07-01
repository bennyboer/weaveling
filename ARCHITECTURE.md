# Weaveling — Architecture

This document is the *how*. For the *what* and the *why*, see [README.md](./README.md).

Nothing here is built yet — this captures the architectural decisions we've settled on so they aren't lost. Where a decision is provisional, it says so.

## Shape

Weaveling is a **client–server application**.

- **Server:** a **modular monolith** written in **Rust**.
- **Client:** a browser. "Write from anywhere with just a browser" is a core goal.
- **Database:** **PostgreSQL**.

The server is a monolith *for now*, but organised into clean modules so that seams can later become process/service boundaries without a rewrite.

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

### Presence is neither

Cursor position, selection, and who's-online are **ephemeral awareness** state. They are **never** event-sourced and **never** persisted — they travel over the WebSocket (Yjs awareness protocol) and are discarded.

## Local-First

Because prose is a CRDT, the client keeps a **full replica in the browser (IndexedDB)**. This gives us, almost for free:

- **offline editing** — the server being down or unreachable never blocks writing,
- **zero typing latency** — edits apply locally and sync in the background,
- a concrete expression of the "your data is yours" value.

The client is designed to be **local-first-capable**, not a thin dumb terminal.

## Event Sourcing — Discipline

- **Be selective.** Event-source only where history has value (the structural domain, undo/redo). Plain state for settings, presence, search indexes, and blobs. "ES where history has value" — not everything.
- **Event store:** rolled on **PostgreSQL** — an events table with per-aggregate streams and optimistic concurrency via a version column.
- **Projections / async work:** a **transactional outbox** + poller for reliable projection updates; PostgreSQL `LISTEN/NOTIFY` (or logical replication) to nudge the WebSocket gateway. No message broker for now (see below).
- **Versioning / upcasting** is planned from day one — a book project lives for years and events will evolve.
- **Snapshots** for aggregates to keep rebuilds fast at scale (a paragraph-per-node book is thousands of nodes).

## Messaging Broker — Deferred (option kept open)

We start **without** a broker (no RabbitMQ/AMQP yet). Inside a modular monolith, in-process dispatch + the Postgres-backed outbox goes a long way and avoids dual-write/ordering complexity before it pays off.

A broker can be introduced later at a real module boundary — e.g. when there are separately-deployed consumers or cross-service async handoff — **without rearchitecting**, provided the module seams stay clean. Keeping this option open is an explicit goal.

## Frontend — Full-Stack Rust (provisional)

We try the **full-stack Rust** path first (e.g. Leptos / Dioxus / Yew compiled to WASM). It's part of the fun and keeps the whole stack in one language.

**Fallback:** if building a collaborative rich-text editor with strong typography proves too heavy in Rust/WASM, we switch the frontend to something like **Angular** (with a proven editor + Yjs bindings), keeping the Rust backend. This is a known, acceptable escape hatch — not a failure.

## Export

Two distinct export pipelines, driven by an in-order walk of the structure:

- **PDF** via **Typst** (Rust-native typesetting, embedded as a library — implement its `World` trait for font/file resolution). Print-quality output.
- **EPUB** via a separate path: structure walk → semantic HTML → EPUB package (reflowable XHTML+CSS). Reuses the web view's HTML rendering. Typst does **not** produce EPUB.

Font bundling/licensing for embedded fonts is a concern for both.

## Supporting Concerns (noted for later)

- **Search & mention detection** — projections built with **Tantivy** (Rust-native, Lucene-like): full-text search over prose, plus codex alias/mention indexing (the "which nodes does this character appear in" back-links).
- **Blobs** (codex and research images) — object storage or filesystem, referenced by ID from events. Not stored as byte columns and not in the event log.
- **Multi-author** — implies accounts, project membership, and permissions. Real scope, acknowledged early.

## Status

Design captured; nothing implemented. The solution is still being woven.
