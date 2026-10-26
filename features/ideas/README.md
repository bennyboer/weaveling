# ideas

An idea is one unit of an author's thinking — captured first, arranged later.

This is where the model lives: **ideas are a pool, and every view is an arrangement of them**. An idea knows its project and its title, and nothing else. It does **not** know where it sits on a board or where it hangs in an outline — position belongs to the view, never to the idea. See [Ideas and views](../../ARCHITECTURE.md#ideas-scenes-and-views--the-non-linear-model).

**`Idea` is an aggregate** — `Captured`, `Retitled`, `Discarded` — and it was the first one, where event sourcing debuted. An idea may be captured untitled, because an author mid-thought should not be stopped to name things.

Discarding is the interesting edge: it leaves the idea refusing everything and drops it from the listing, and `idea.discarded` is what the board listens for so it can unpin it. That stays housekeeping rather than correctness — a client must keep tolerating a dangling reference, because any transport leaves a window.

**One way in: `IdeasCore`.** The feature's core is entered through one struct, whose methods are everything ideas does: what someone does (listing, capturing, reading, retitling, discarding, following), and what the feature does itself (cataloguing, the sweep after a project is deleted, finding an idea's project for the broadcast). Every permission is asked there and nowhere else; the core hands on to `IdeaService`, which only carries out the commands, records who acted and cannot be reached from outside the core. A write answers with the idea as written. Its errors are one enum, `IdeasError`. The adapters only translate.

**Ideas are there only for those in their project.** Listing a project's ideas, reading one and following them live ask the [`Authorizer`](../../libraries/authorizing) for `Read`; capturing, retitling and discarding for `Write`, an author's. An idea knows its project, so the core reads it first and asks about that, before anything else of the request is judged: a stranger sending a title too long is told the idea is not there, not that the title is too long. Whoever may not read the project is answered 404, as for an idea or a project not there; whoever may read but not write is told, 403 `not-an-author`.

**Crates:** `core` (`IdeasCore`, aggregate, catalog port, service) · `contract` · `adapters/store` (stored-event codec) · `adapters/catalog` (listings per project) · `adapters/messaging` (event publisher and catalog projector) · `adapters/rest` · `wiring` · `tests`.
