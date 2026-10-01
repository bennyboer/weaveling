# boards

The spatial view: ideas pinned on a canvas, where an author can see them all at once.

A board holds **placements** — an idea, a spot, a size — and nothing else about the idea. The title an author reads on a card comes from the [idea catalog](../ideas), joined in the client. That is deliberate: a board that stored titles would hold a second copy of a fact it does not own, and every retitle would have to chase it. See [the frontend join](../../ARCHITECTURE.md#the-board-renders-through-a-frontend-join).

**`Board` is an aggregate** — `Started`, `IdeaPinned`, `IdeaMoved`, `IdeaResized`, `IdeaRaised`, `IdeaUnpinned`. Moving and resizing are separate events even though one `Reshape` command can do both, because an event says what changed rather than what a caller asked for. Multiple boards per project are in the model from the first event even though the first version ships one, because keying placements by project id is the shortcut that would turn a second board into a migration of everything ever written.

**A project cannot start its own board**, since a feature may not call another feature. So a board is started on first open — find-or-start, under one write guard — and a project may legitimately have no board yet. Anything sweeping a project has to tolerate that.

**Two projectors and a listener.** `catalogue-board` keeps the board summaries; `index-pinned-ideas` keeps the index of which boards hold a given idea; `unpin-discarded-idea` hears `idea.discarded` and takes it off every board holding it.

**Crates:** `core` (aggregate, catalog port, service, `Spot`/`Size`) · `contract` · `adapters/store` · `adapters/catalog` · `adapters/messaging` (event publisher, catalog projector, discard listener) · `adapters/rest` · `wiring` · `tests`.
