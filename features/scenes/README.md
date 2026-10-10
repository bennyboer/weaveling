# scenes

The prose itself — the one part of Weaveling that is **not** event-sourced.

A scene is a CRDT document (`yrs`, the Rust side of Yjs), because prose is edited character by character by people who may be offline, and that is exactly what an event log is bad at. This is the other half of the [two-speed model](../../ARCHITECTURE.md#prose--the-editing-stack): event sourcing for structure, CRDTs for text.

**The server is a participant, not a relay.** A scene being edited is held as a `LiveScene` with its own document, and every update is applied to it. That is what lets a peer who was offline for a week be caught up by the server rather than by whichever other client happens to be connected — and it is where persistence and compaction attach.

**Storage is an append-only log of updates**, compacted into a snapshot when it grows. Appends commute, so concurrent applies need no locking; a read-merge-write backend would silently lose one of two concurrent updates, below the merge where CRDTs cannot help.

**A scene belongs to a project, and to nothing else.** Views will point at prose — the outline arranges it, an idea may name it — but ownership is a column on the scene itself, and the project’s deletion sweep is the only thing that removes one. Nothing an author throws away can take prose with it: an idea is disposable, a scene is the book.

**Scenes announce, though they are not event-sourced.** Linking an idea, unlinking one and deleting a scene are told to the rest of the app as `scene.idea.linked`, `scene.idea.unlinked` and `scene.deleted`, written into the scenes outbox **in the same transaction as the change** — so a change that rolls back is never announced, and one that commits always is. Only a real change is announced: linking an idea twice says so once. Writing and retitling announce nothing; they concern no one outside.

**A scene counts its changes.** It has no event stream to number them, so it keeps a `version` that every change bumps in the same transaction — writing, retitling, linking, unlinking; compaction is not a change, it only rewrites storage. Link messages carry it, because a listener needs it to tell a link tried again late from the unlink that followed it. Bumping locks the scene's row, so two changes racing on one scene get their numbers in the order they commit. Deletion needs no number: nothing comes back from it.

Awareness — cursors, selections, who is here — is relayed as opaque bytes and never decoded or stored. Presence is ephemeral by decision, expressed as a dependency arrow: it lives and dies inside `adapters/sync`.

**Crates:** `core` (`Scene`, store port, service) · `contract` · `adapters/store` (in memory, PostgreSQL and SQLite; append-only plus compaction) · `adapters/sync` (the `y-websocket` server: sockets, peers, protocol, live scenes) · `adapters/rest` · `wiring` · `tests`.
