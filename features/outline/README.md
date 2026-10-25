# outline

The linear view: the tree an author shapes a book into.

**The outline arranges sections, not content.** A section is a place in the book with a title of its own; a scene or an idea is *attached* to a section. That indirection is what keeps content off the tree — a section can exist before anything fills it, and a scene can be attached, detached and reattached without the tree changing shape. See [the rule in full](../../ARCHITECTURE.md#the-outline-arranges-sections-not-scenes).

**`Outline` is an aggregate** — `Started`, `SectionAdded`, `SectionRetitled`, `SectionMoved`, `SectionPromoted`, `SectionDemoted`, `SectionRemoved`, `Attached`, `Detached`. Sections are added relative to their neighbours (`under`, `after`) rather than at an index, because an index is a fact about the list at one moment and would be wrong for anyone who reordered concurrently.

**A move announces where it left.** Attaching something that already sits in a section emits `Detached` and then `Attached`, rather than a lone `Attached` that only names where it went — and every `Detached` names the section it left (`from`), whether it was a move or a plain detach. Anyone keeping their own record of where things sit — [appearances](../appearances) does — learns of the old section from the detach, and needs no memory of their own to find it.

**A section holds a tagged `Attachment`, not a bare id** — `Scene(id) | Idea(id)`, in one ordered list. A scene is the book’s text; an idea pinned beside it is a **note to the author**, which is why `reading_order()` walks straight past ideas: exporting the book must never pick up the sticky notes. The tag is part of the identity, so an idea and a scene whose ids happen to match are two different attachments.

Like a board it is started on first open, and like a board it carries two projectors and a listener: `catalogue-outline` for the tree itself, `index-attachments` for which outlines hold a given attachment, and `detach-discarded-idea` for a note whose idea has gone away. There is no equivalent for a scene — a scene is only ever removed with its project, and that takes the whole outline with it.

**An outline is there only for those in its project.** Opening a project's outline, reading it and following it live ask the [`Authorizer`](../../libraries/authorizing) for `Read`; adding, retitling, placing, promoting, demoting and removing sections, attaching and detaching for `Write`, an author's. The service reads the outline first and asks about its project. Whoever may not read the project is answered 404, as for one not there, and the live route too, before it upgrades; whoever may read but not write is told, 403 `not-an-author`. A reader opening a project's outline before anyone has starts it, as opening always has. What the feature does itself, as `Agent::System`, is let through by the authorizer.

**What went leaves a tombstone.** `detach-discarded-idea` and `detach-deleted-scene` mark the idea or the scene gone in `gone_attachments`, so an attachment indexed after it went is still taken out, and keep `gone_at`, the `occurred_at` of the message. The chore `forget-old-outline-tombstones`, a [`ForgetOldTombstones`](../../libraries/chores), forgets them once no message about them can still arrive, in batches; an attachment of something gone longer ago than that, from a stale or forged request, is no longer taken out.

A section title may be blank — an author sketching structure should not be forced to name a chapter before they know what is in it.

**Crates:** `core` (aggregate, catalog port, service, `SectionTitle`) · `contract` · `adapters/store` · `adapters/catalog` · `adapters/messaging` (event publisher, catalog projector, attach and discard listeners) · `adapters/rest` · `wiring` · `tests`.
