# outline

The linear view: the tree an author shapes a book into.

**The outline arranges sections, not content.** A section is a place in the book with a title of its own; a passage or an idea is *attached* to a section. That indirection is what keeps content off the tree — a section can exist before anything fills it, and a passage can be attached, detached and reattached without the tree changing shape. See [the rule in full](../../ARCHITECTURE.md#the-outline-arranges-sections-not-pieces).

**`Outline` is an aggregate** — `Started`, `SectionAdded`, `SectionRetitled`, `SectionMoved`, `SectionPromoted`, `SectionDemoted`, `SectionRemoved`, `Attached`, `Detached`. Sections are added relative to their neighbours (`under`, `after`) rather than at an index, because an index is a fact about the list at one moment and would be wrong for anyone who reordered concurrently.

**A section holds a tagged `Attachment`, not a bare id** — `Passage(id) | Idea(id)`, in one ordered list. A passage is the book’s text; an idea pinned beside it is a **note to the author**, which is why `reading_order()` walks straight past ideas: exporting the book must never pick up the sticky notes. The tag is part of the identity, so an idea and a passage whose ids happen to match are two different attachments.

Like a board it is started on first open, and like a board it carries two projectors and a listener: `catalogue-outline` for the tree itself, `index-attachments` for which outlines hold a given attachment, and `detach-discarded-idea` for a note whose idea has gone away. There is no equivalent for a passage — a passage is only ever removed with its project, and that takes the whole outline with it.

A section title may be blank — an author sketching structure should not be forced to name a chapter before they know what is in it.

**Crates:** `core` (aggregate, catalog port, service, `SectionTitle`) · `contract` · `adapters/store` · `adapters/catalog` · `adapters/messaging` (event publisher, catalog projector, attach and discard listeners) · `adapters/rest` · `wiring` · `tests`.
