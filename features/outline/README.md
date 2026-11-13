# outline

The linear view: the tree an author shapes a book into.

**The outline arranges sections, not content.** A section is a place in the book with a title of its own; a scene or an idea is *attached* to a section. That indirection is what keeps content off the tree — a section can exist before anything fills it, and a scene can be attached, detached and reattached without the tree changing shape. See [the rule in full](../../ARCHITECTURE.md#the-outline-arranges-sections-not-scenes).

**`Outline` is an aggregate** — `Started`, `SectionAdded`, `SectionRetitled`, `SectionMoved`, `SectionPromoted`, `SectionDemoted`, `SectionRemoved`, `Attached`, `Detached`. Sections are added relative to their neighbours (`under`, `after`) rather than at an index, because an index is a fact about the list at one moment and would be wrong for anyone who reordered concurrently.

**A move announces where it left.** Attaching something that already sits in a section emits `Detached` and then `Attached`, rather than a lone `Attached` that only names where it went — and every `Detached` names the section it left (`from`), whether it was a move or a plain detach. Anyone keeping their own record of where things sit — [appearances](../appearances) does — learns of the old section from the detach, and needs no memory of their own to find it.

**A section holds a tagged `Attachment`, not a bare id** — `Scene(id) | Idea(id)`, in one ordered list. A scene is the book’s text; an idea pinned beside it is a **note to the author**, which is why `reading_order()` walks straight past ideas: exporting the book must never pick up the sticky notes. The tag is part of the identity, so an idea and a scene whose ids happen to match are two different attachments.

Like a board, an outline is started on first open. Also like a board, it has two projectors and some listeners for things that went away:

- `catalogue-outline` keeps the summary of the tree itself.
- `index-attachments` keeps the index of which outlines hold a given attachment.
- `detach-discarded-idea` receives that an idea was discarded, and `detach-deleted-scene` that a scene was deleted.
- `detach-everywhere` and `detach-gone-attachments` take the gone attachment out. The other listeners trigger them.

**Heard once, or safe to hear again.** `catalogue-outline` receives its messages through [`inbox::Inboxed`](../../libraries/inbox/README.md). The outline's summary and the record that the message was received are committed in one transaction. The chore `forget-old-outline-hearings` deletes these records once the tombstones' time is over. `index-attachments`, `detach-discarded-idea`, `detach-deleted-scene`, `detach-everywhere` and `detach-gone-attachments` also use the inbox. Only the sweep `discard-outlines-of-deleted-project` stays a plain listener.

**A deleted project opens no outline.** `outline-remember-deleted-projects` receives the deletion through the inbox. It stores the project's tombstone in the catalog, in `project_tombstones`. In the same transaction, it asks for the sweep with `follow-up.outline.sweep-project`. `discard-outlines-of-deleted-project` receives this follow-up instead of `project.deleted`. So it reads the registry only after the tombstone is committed.

`open` refuses a project with a tombstone, as if the project were not there. After its own claim on the outline is committed, `open` checks again. If a tombstone was stored in the meantime, it discards the outline it started. So when an open and a deletion race, either the open sees the tombstone, or the sweep sees the outline. The sweep does nothing if the outline is not there or is already discarded. The chore that deletes the other tombstones also deletes the projects' tombstones.

**An attachment racing what it points at going is still taken out, by a follow-up.** Two listeners may run at the same time: one writes the index of an outline's attachments, the other writes the tombstone of an idea or a scene. Each must see what the other committed. If each read in its own transaction, either could miss the other. So each one writes, and in the same transaction puts a follow-up in the outbox. The outbox sends the follow-up only after that transaction is committed:

- `detach-discarded-idea` and `detach-deleted-scene` send `follow-up.outline.detach-everywhere` to `detach-everywhere`. That listener detaches the attachment from every outline that holds it, according to the index.
- `index-attachments` sends `follow-up.outline.detach-gone` to `detach-gone-attachments`. That listener detaches the first attachment in the outline that has a tombstone.

When two writers race, the follow-up of the later one reads what the earlier one committed.

The follow-ups' routing keys begin with `follow-up.`, so no `outline.#` subscription receives them. They are defined in the messaging adapter, not in the contract. `detaching.rs` holds both listeners that send `detach-everywhere`, and the listener that receives it. `attaching.rs` holds the index and `detach-gone-attachments`.

An outline removes only one gone attachment per message, because two detaches on one outline in one transaction would collide. The detach creates its own `outline.detached`. The index receives it and asks the outline again, until no gone attachment is left. Every step is one transaction, the one of the listener handling the message. Each step is idempotent: an attachment that is not there, or an outline that is discarded, is not a failure.

**An outline is there only for those in its project.** Opening a project's outline, reading it and following it live ask the [`Authorizer`](../../libraries/authorizing) for `Read`. These ask for `Write`, which an author has: adding, retitling, placing, promoting, demoting and removing sections, and attaching and detaching. The service reads the outline first and then asks about its project. Someone who may not read the project gets a 404, the same as for an outline that is not there. The live route does the same, before it upgrades the connection. Someone who may read but not write gets 403 `not-an-author`. A reader who opens a project's outline before anyone else has done so starts it, as opening always did. What the feature does itself, as `Agent::System`, is allowed by the authorizer.

**Who may do what is kept by the feature itself,** through [`access`](../../libraries/access/README.md). This data is stored in the outline's own database. Two listeners receive their messages through the inbox:

- `RememberRolesListener` receives the members' events (`founded`, `joined`, `role.changed`, `left`, `removed`, `disbanded`).
- `RememberTrashListener` receives when projects are put in the trash and restored.

A chore deletes old tombstones. If there are accounts, the core asks `KeptAccess` who may do what. If a follower loses access, the live connection is closed once the listener's change is committed. Without accounts, everyone is allowed.

**An outline attaches only ideas and scenes of its own project.** `outline-remember-projects-of-attachments` receives `idea.captured` and `scene.created`. It stores the project of each idea and scene in the [`belonging`](../../libraries/belonging/README.md) store. `detach-discarded-idea` and `detach-deleted-scene` remove that entry when they store the tombstone. So nothing of a deleted project's ideas and scenes remains. `attach` refuses an idea or a scene that is known to belong to another project with 404 `not-found`, and the answer tells nothing about it. An idea or scene the outline has not heard of yet is attached. This is because it may have been created a moment ago, before the message about its creation arrives (*13c-2d, 2026-11-11*).

**What went leaves a tombstone.** `detach-discarded-idea` and `detach-deleted-scene` mark the idea or the scene as gone in `attachment_tombstones`. So an attachment that is indexed after it went is still taken out. The tombstone stores `laid_at`, which is the `occurred_at` of the message. The chore `forget-old-outline-tombstones`, a [`ForgetOldTombstones`](../../libraries/chores), deletes the tombstones in batches once no message about them can arrive anymore. After that, an attachment of something that went that long ago, from an outdated or forged request, is no longer taken out.

A section title may be blank — an author sketching structure should not be forced to name a chapter before they know what is in it.

**A outline forgotten stays forgotten.** The catalog removes a discarded outline in one transaction: it stores a tombstone in `outline_tombstones` and deletes the outline's summary. `remember` refuses an outline with a tombstone, so a summary that arrives late cannot bring the outline back.

On PostgreSQL, a `remember` may read no tombstone just before a forget commits one. Then the `remember` loses the race, because both transactions are serializable. It is tried again and finds the tombstone. (*The lock was removed on 2026-11-04, when every PostgreSQL transaction became serializable.* Before that, there was a lock on the outline.) On SQLite, only one writer at a time keeps them apart.

`forget-old-outline-tombstones` deletes these tombstones together with the attachment tombstones. A summary holds only the outline and its project, and the project never changes. So it needs no version guard.

**Core owns each write's transaction.** `OutlinesCore` begins the transaction of every write after it has asked for permission. A change runs through `transacting::retrying!`. After a lost race, a change that names no version is tried up to five times, and a change that names a version is tried once (`eventsourcing::tries_for(expected)`). So that conflict is answered with a 412. The system's own changes name no version. Opening claims the outline and starts it in one transaction, so a claim never points at nothing. Cataloguing, recording what an outline holds, storing a tombstone and deleting old tombstones each begin their own transaction, and the catalog's writes use it.

**One way in: `OutlinesCore`.** Everything goes through this one struct:

- opening, reading, following, and every change to the sections and what they hold,
- and what the feature does itself: cataloguing, recording what each outline holds, removing a discarded idea or a deleted scene, and the sweep after a project is deleted.

It asks for `Read` or `Write` on the outline's project. The system's own work is done, and recorded, as `Agent::System`. Only the sweep discards an outline. A change answers with the outline as written. `OutlineService` only carries out the commands, in the transaction that `OutlinesCore` hands it, and nothing outside the core can reach it. Errors are one enum, `OutlinesError`.

**Crates:** `core` (`OutlinesCore`, aggregate, catalog port, service, `SectionTitle`) · `contract` · `adapters/store` · `adapters/catalog` · `adapters/messaging` (event publisher, catalog projector, attach and discard listeners) · `adapters/rest` · `wiring` · `tests`.
