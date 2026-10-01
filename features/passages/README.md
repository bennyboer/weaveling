# passages

The prose itself — the one part of Weaveling that is **not** event-sourced.

A passage is a CRDT document (`yrs`, the Rust side of Yjs), because prose is edited character by character by people who may be offline, and that is exactly what an event log is bad at. This is the other half of the [two-speed model](../../ARCHITECTURE.md#prose--the-editing-stack): event sourcing for structure, CRDTs for text.

**The server is a participant, not a relay.** A passage being edited is held as a `LivePassage` with its own document, and every update is applied to it. That is what lets a peer who was offline for a week be caught up by the server rather than by whichever other client happens to be connected — and it is where persistence and compaction attach.

**Storage is an append-only log of updates**, compacted into a snapshot when it grows. Appends commute, so concurrent applies need no locking; a read-merge-write backend would silently lose one of two concurrent updates, below the merge where CRDTs cannot help.

**A passage belongs to a project, not to whatever points at it.** The prose is reachable from an idea, and later from the outline, but ownership is a column on the passage itself: deleting a project has to reach the words directly. If the only path to prose ran through its idea, dropping that link would leave a book nobody can find and nothing can collect.

Awareness — cursors, selections, who is here — is relayed as opaque bytes and never decoded or stored. Presence is ephemeral by decision, expressed as a dependency arrow: it lives and dies inside `adapters/sync`.

**Crates:** `core` (`Passage`, store port, service) · `contract` · `adapters/store` (in-memory and PostgreSQL, append-only plus compaction) · `adapters/sync` (the `y-websocket` server: sockets, peers, protocol, live passages) · `adapters/rest` · `wiring` · `tests`.
