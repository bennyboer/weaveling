# boards

The spatial view: ideas pinned on a canvas, where an author can see them all at once.

A board holds **placements** — an idea, a spot, a size — and nothing else about the idea. The title an author reads on a card comes from the [idea catalog](../ideas), joined in the client. That is deliberate: a board that stored titles would hold a second copy of a fact it does not own, and every retitle would have to chase it. See [the frontend join](../../ARCHITECTURE.md#the-board-renders-through-a-frontend-join).

**`Board` is an aggregate** — `Started`, `IdeaPinned`, `IdeaMoved`, `IdeaResized`, `IdeaRaised`, `IdeaUnpinned`. Moving and resizing are separate events even though one `Reshape` command can do both, because an event says what changed rather than what a caller asked for. Multiple boards per project are in the model from the first event even though the first version ships one, because keying placements by project id is the shortcut that would turn a second board into a migration of everything ever written.

**A project cannot start its own board**, since a feature may not call another feature. So a board is started on first open — find-or-start, under one write guard — and a project may legitimately have no board yet. Anything sweeping a project has to tolerate that.

**Two projectors and a listener.** `catalogue-board` keeps the board summaries; `index-pinned-ideas` keeps the index of which boards hold a given idea; `unpin-discarded-idea` hears `idea.discarded` and takes it off every board holding it.

**A board is there only for those in its project.** Opening a project's board, reading it and following it live ask the [`Authorizer`](../../libraries/authorizing) for `Read`; pinning, moving, resizing and unpinning for `Write`, an author's. The service reads the board first and asks about its project. Whoever may not read the project is answered 404, as for one not there, and the live route too, before it upgrades; whoever may read but not write is told, 403 `not-an-author`. A reader opening a project's board before anyone has starts it, as opening always has. What the feature does itself, as `Agent::System`, is let through by the authorizer.

**A discarded idea leaves a tombstone.** `idea_tombstones` keeps every idea heard discarded, so a pin indexed after the discard is still let go of, and keeps `laid_at`, the `occurred_at` of the discard. The chore `forget-old-board-tombstones`, a [`ForgetOldTombstones`](../../libraries/chores), forgets it once no message about the idea can still arrive, in batches. A pin of an idea discarded longer ago than that, from a stale or forged request, is no longer let go of; the board keeps it, and the client, which draws only ideas it knows, does not show it.

**Crates:** `core` (aggregate, catalog port, service, `Spot`/`Size`) · `contract` · `adapters/store` · `adapters/catalog` · `adapters/messaging` (event publisher, catalog projector, discard listener) · `adapters/rest` · `wiring` · `tests`.
