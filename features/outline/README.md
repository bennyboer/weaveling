# outline

The linear view: the tree an author shapes a book into.

**The outline arranges sections, not pieces.** A section is a place in the book with a title of its own; a piece is *attached* to a section. That indirection is what keeps content off the tree — a section can exist before anything fills it, and a piece can be attached, detached and reattached without the tree changing shape. See [the rule in full](../../ARCHITECTURE.md#the-outline-arranges-sections-not-pieces).

**`Outline` is an aggregate** — `Started`, `SectionAdded`, `SectionRetitled`, `SectionMoved`, `SectionPromoted`, `SectionDemoted`, `SectionRemoved`, `PieceAttached`, `PieceDetached`. Sections are added relative to their neighbours (`under`, `after`) rather than at an index, because an index is a fact about the list at one moment and would be wrong for anyone who reordered concurrently.

Like a board it is started on first open, and like a board it carries two projectors and a listener: `catalogue-outline` for the tree itself, `index-attached-pieces` for which section holds a given piece, and `detach-discarded-piece` for a piece that has gone away.

A section title may be blank — an author sketching structure should not be forced to name a chapter before they know what is in it.

**Crates:** `core` (aggregate, catalog port, service, `SectionTitle`) · `contract` · `adapters/store` · `adapters/catalog` · `adapters/messaging` (event publisher, catalog projector, attach and discard listeners) · `adapters/rest` · `wiring` · `tests`.
