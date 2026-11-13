# scenes

The prose itself — the one part of Weaveling that is **not** event-sourced.

A scene is a CRDT document (`yrs`, the Rust side of Yjs), because prose is edited character by character by people who may be offline, and that is exactly what an event log is bad at. This is the other half of the [two-speed model](../../ARCHITECTURE.md#prose--the-editing-stack): event sourcing for structure, CRDTs for text.

**The server is a participant, not a relay.** A scene being edited is held as a `LiveScene` with its own document, and every update is applied to it. That is what lets a peer who was offline for a week be caught up by the server rather than by whichever other client happens to be connected — and it is where persistence and compaction attach.

**Storage is an append-only log of updates**, compacted into a snapshot when it grows.
Appends commute, so concurrent applies need no locking among themselves.
A read-merge-write backend would silently lose one of two concurrent updates. That loss would happen below the merge, where CRDTs cannot help.
**An apply and a compaction must not interleave**, though.
Compaction takes a snapshot of what it reads and deletes what came before.
So an update numbered before the snapshot, but committed after it, would be lost for good.
On PostgreSQL, both take the scene's row lock first.
`apply` takes it in a statement of its own, before it numbers its update.
So whichever comes second waits for the first, and sees what the first wrote.
On SQLite, only one writer runs at a time, which has the same effect.
A test sends two hundred updates and fifty compactions at once, and finds every one of the updates in the prose.

**A scene belongs to a project, and to nothing else.** Views will point at prose — the outline arranges it, an idea may name it — but ownership is a column on the scene itself, and the project’s deletion sweep is the only thing that removes one. Nothing an author throws away can take prose with it: an idea is disposable, a scene is the book.

**Scenes announce, though they are not event-sourced.** Linking an idea, unlinking one and deleting a scene are told to the rest of the app as `scene.idea.linked`, `scene.idea.unlinked` and `scene.deleted`, written into the scenes outbox **in the same transaction as the change** — so a change that rolls back is never announced, and one that commits always is. Only a real change is announced: linking an idea twice says so once. Writing and retitling announce nothing; they concern no one outside.

**A scene counts its changes.**
It has no event stream to number them.
So it keeps a `version` that every change increases, in the same transaction as the change.
The changes are writing, retitling, linking and unlinking.
Compaction is not a change; it only rewrites storage.
Link messages carry the version.
A listener needs it to tell a link that was retried late apart from the unlink that followed it.
Increasing the version locks the scene's row.
So when two changes race on one scene, they get their numbers in the order they commit.
Deletion needs no number, because nothing comes back after it.

**One way in: `ScenesCore`.**
The feature's core is entered through one struct, as every feature's core is.
Its methods are everything the scenes feature does.
Every permission is checked there, and nowhere else.
The core asks the `Authorizer`, then hands on to `SceneService` and to the ports behind it.
`SceneService` only does the work. It knows nobody and cannot be reached from outside the core.
The adapters only translate.
Its errors are one enum, `ScenesError`.
Every adapter goes through it: the REST routes, both live topics, the editing socket, the listeners (cataloguing, sweeping, unlinking, restyling) and the chores that forget old tombstones and old records of received messages.

**A scene links only ideas of its own project.**
`scenes-remember-projects-of-ideas` receives `idea.captured`.
It stores the idea's project in the [`belonging`](../../libraries/belonging/README.md) store.
`unlink-discarded-idea` forgets it when it unlinks the idea everywhere.
So nothing of a deleted project's ideas stays.
`link` refuses an idea that is known to be of another project, as if it were not there: 404 `not-found`. It reveals nothing about it.
An idea the feature has not received yet is linked.
That is because an idea captured a moment ago may be linked before its capture message arrives (*13c-2c, 2026-11-11*).

**Heard once.**
`catalogue-scene`, `unlink-discarded-idea` and `scenes-remember-projects-of-ideas` are `KeptHandler`s run through [`inbox::Inboxed`](../../libraries/inbox/README.md).
It begins the transaction, skips a message the feature's inbox has received before, and hands the transaction on.
`catalogue_created`, `catalogue_retitled`, `catalogue_opening`, `catalogue_deleted` and `unlink_everywhere` take it as `within` and join it.
So the change and the record of having received the message commit together.
A scene that is compacted after unlinking is compacted once that has committed.
The sweep `delete-scenes-of-deleted-project` and `restyle-quotation-marks-of-project` stay plain `Handler`s.
They span many transactions on purpose, one per batch.
Continuation messages carry them on from batch to batch.
Each of their steps is safe to repeat.
The chore `forget-old-scene-hearings` forgets the old records of received messages.

**A restyle counts each page once and carries on once.**
*Fixed on 2026-11-02, after the transaction audit. Before, a page received again claimed the book again, a first message received again reset `done`, and `done` grew past `total`.*
The task keeps a `cursor`: the last scene that its counted pages reached.
`begin` begins only a newer change, or a book that was never restyled.
So the same change received again starts nothing over.
`standing` returns the task's change, its cursor, and whether it is finished.
From that, the core decides what to do with a page:

- A page whose `after` is where the cursor stands is restyled.
- A page of a change that was superseded is skipped.
- A page the cursor has already passed is not restyled. It carries on from the cursor. So a page whose ack was lost sends its continuation again, and the chain does not stop.

`progressed` is a compare-and-set.
It counts the page and moves the cursor only while the cursor still stands where the page began, and the task is not finished.
It returns whether it counted.
Only a page that counted carries on.
So if two copies of one page are handled at once, the page is counted once and no second chain starts.
A continuation's id is derived from the message it answers: a SHA-256 of `carry-on-restyling:` and that message's id, as a UUIDv8.
So a page received again answers with the very same continuation it sent before.
A page may keep failing until its delivery gives up.
The task then stays `finished = false`, with its count where it stood.
`GET /scenes/quotation-restylings/{project}` and the live topic show this.
If the dead letter is retried, the restyle continues from the cursor.

**A project's scenes are there only for those in it.**
These ask the [`Authorizer`](../../libraries/authorizing) for `Read`:
listing a project's scenes, reading one, asking how far its quotation marks are restyled, and following either of these live.
These ask for `Write`, which is an author's right:
creating, retitling, linking, unlinking and deleting. The core opens the scene first, and asks about its project.
Someone who may not read the project is answered 404, as for a scene that is not there.
The live routes do the same, before they upgrade.
Someone who may read but not write is told so, 403 `not-an-author`.
What the feature does itself, the sweep after a project is deleted, acts as `Agent::System`. The authorizer lets it through.
Opening and applying do not ask, because the editing socket asks when a scene is joined.
Joining a scene checks who opened the socket: an author, a reader or a stranger.
A stranger is refused the scene by name, as if it were not there.
Everyone is refused in the same way while the authorizer cannot be asked.
A reader may join.
A reader is told every change, and may ask for what was missed.
The others see where a reader's cursor is.
But what a reader sends to change the text is dropped. It is neither stored nor passed on.
**And asks again while joined.**
Each joined scene watches its project.
When the project's members change, the connection asks again for that scene.
So a member who was removed is refused the scene, as on joining.
A changed role takes effect at once.
The live routes close with 4403 once the follower may no longer read.

**Who may do what is kept by the feature itself,**
through [`access`](../../libraries/access/README.md), stored in the scenes' own database.
`RememberRolesListener` receives the members' messages (`founded`, `joined`, `role.changed`, `left`, `removed`, `disbanded`).
`RememberTrashListener` receives the messages about projects put in the trash and restored.
Both run through the inbox.
A chore forgets the tombstones.
Where there are accounts, the core asks `KeptAccess` who may do what. The editing socket asks through the core.
A joined connection asks again once the listener's change is committed.
Without accounts, everyone is allowed.

**A deleted scene stays out of the catalog.**
The catalog keeps a tombstone of each deleted scene in `scene_tombstones`.
So a title or an opening that arrives late does not bring the scene back.
The tombstone also stores `laid_at`, the deletion's `occurred_at`.
The chore `forget-old-scene-tombstones`, a [`ForgetOldTombstones`](../../libraries/chores), forgets it in batches, once no message about the scene can still arrive.
On PostgreSQL, an upsert (remembering, retitling or noting an opening) may read no tombstone before a forget commits one.
That upsert then loses the race and is tried again.
(*The lock was removed on 2026-11-04, when every PostgreSQL transaction became serializable.* Before that, a lock on the scene did this.)
On SQLite, only one writer runs at a time, which has the same effect. In memory, the feature's one lock does.

**A count clears only what it counted.**
A live scene runs one word count at a time, behind a lock of its own.
So a count that stalls is never reported after a newer one.
The count reads the scene's `version` before it reads the words and the opening.
`announce_count` reports the words, and the opening if it changed, in one transaction.
It clears the scene's `uncounted` mark only `WHERE version` is still the one it read.
So an edit stored while the count was running keeps the mark.
If the process stops first, `count-scenes-left-uncounted` counts the scene again.
Retitling and linking also move the version.
So one of them landing during a count costs one more count, nothing worse.

**An edit the store refused is not left for eviction.**
If a live scene's edit could not be stored, the scene is marked unsaved.
The next edit that is stored then stores the whole document, not only itself.
So a crash before the scene is evicted loses nothing that was stored after that.
A restyling may find a live scene that is already in the right style.
If that scene is unsaved, the restyling stores it whole first.
If storing fails, the restyling answers with the failure.
So the restyling's cursor never moves past a restyle that the store never received.

**A deleted project takes no new scene.**
`delete-scenes-of-deleted-project` lays the project's tombstone in `project_tombstones` before it sweeps.
`laid_at` is the deletion's `occurred_at`.
It does this only for the deletion message itself, not for its continuations.
`ScenesCore::create` checks for the tombstone in the transaction it writes in, `has_project_tombstone_within`.
If it finds one, it answers `ProjectNotThere`.
So nothing that a stale tab writes in local mode, where every action is allowed, outlives the project.
On SQLite and in memory, the tombstone lands either before the create or after it.
On PostgreSQL, a create that read no tombstone loses the race against the tombstone and the sweep that reads the scenes. It is then tried again.
*Since 2026-11-04.* Until then, create checked before it wrote, and again after the commit, deleting what it had made.
A crash between the two left that scene standing.
The same chore forgets the project tombstones together with the scenes' tombstones, in batches.

Awareness — cursors, selections, who is here — is relayed as opaque bytes and never decoded or stored. Presence is ephemeral by decision, expressed as a dependency arrow: it lives and dies inside `adapters/sync`.

**Core owns each write's transaction.**
`Ports` carries the feature's own `Transactions`.
`ScenesCore` begins one transaction for every write to the store:
creating, applying an update, retitling, linking and unlinking, deleting, and announcing a word count together with an opening that changed.
`SceneService` carries out the write in it.
A scene is read back only once the transaction has committed.
The store's writes take the transaction, so a write and the message it enqueues commit together.
This holds in memory too: the in-memory store now undoes its writes on a rollback, and enqueues through `enqueue_after_commit`.
Compaction stays the store's own job.
When an update leaves the log long enough, it asks the transaction for an `after_commit` that compacts in a transaction of its own.
So compaction never runs before the update has committed.
A compaction that fails is logged, and tried again after the next update.
Nothing is retried, because no write here aims at a version.

**A scene owes its count until it is told.**
*Fixed on 2026-11-02, after the transaction audit. Before, a crash in the two seconds between storing the prose and announcing its count lost both the count and the opening, until the scene's next edit.*
The store marks a scene `uncounted` in the transaction that applies an update.
Announcing its word count clears the mark, in its own transaction.
So the debt is committed together with the prose, and paid only together with the count.
`uncounted(at_most)` lists the scenes that still owe a count.
The chore `count-scenes-left-uncounted` runs every minute, a hundred scenes at a time.
It joins each scene through `LiveScenes` and counts it there.
It counts the words and, because a rehydrated scene has reported no opening, the opening too.
So a scene left uncounted, by a crash or by a count that failed, is counted without waiting for another edit.
A scene can be evicted before its count was reported. This still happens to a scene whose count failed.
Such a scene is caught up the same way.
Counting through the live copy, which holds every stored update, never clears the mark of an update it has not seen.
The catalog's writes, and the restyling tasks' `begin`, `standing` and `progressed`, take the transaction too.
Each cataloguing and each step of a restyle runs in a transaction of its own.
`TellingScenes` and `TellingRestylingTasks` read what they report and report it in an `after_commit`.
So a write that was rolled back is reported to nobody.

**Crates:** `core` (`ScenesCore`, `Scene`, the ports, the service) · `contract` · `adapters/store` (in memory, PostgreSQL and SQLite; append-only plus compaction) · `adapters/sync` (the `y-websocket` server: sockets, peers, protocol, live scenes) · `adapters/rest` · `wiring` · `tests`.
