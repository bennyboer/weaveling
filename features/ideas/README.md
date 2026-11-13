# ideas

An idea is one unit of an author's thinking — captured first, arranged later.

This is where the model lives: **ideas are a pool, and every view is an arrangement of them**. An idea knows its project and its title, and nothing else. It does **not** know where it sits on a board or where it hangs in an outline — position belongs to the view, never to the idea. See [Ideas and views](../../ARCHITECTURE.md#ideas-scenes-and-views--the-non-linear-model).

**`Idea` is an aggregate** — `Captured`, `Retitled`, `Discarded` — and it was the first one, where event sourcing debuted. An idea may be captured untitled, because an author mid-thought should not be stopped to name things.

Discarding is the interesting edge: it leaves the idea refusing everything and drops it from the listing, and `idea.discarded` is what the board listens for so it can unpin it. That stays housekeeping rather than correctness — a client must keep tolerating a dangling reference, because any transport leaves a window.

**A idea forgotten stays forgotten.** The catalog removes a discarded idea in one transaction: it stores a tombstone in `idea_tombstones` and deletes the idea's summary. `remember` refuses an idea with a tombstone. It also replaces a summary only with a newer version. So a summary that arrives late cannot bring the idea back.

On PostgreSQL, a `remember` may read no tombstone just before a forget commits one. Then the `remember` loses the race, because both transactions are serializable. It is tried again and finds the tombstone. (*The lock was removed on 2026-11-04, when every PostgreSQL transaction became serializable.* Before that, there was a lock on the idea.) On SQLite, only one writer at a time keeps them apart.

The chore `forget-old-idea-tombstones` deletes the tombstones one day after a change about the idea can no longer be retried. Every feature handles its tombstones this way.

**A deleted project takes no new idea.** `discard-ideas-of-deleted-project` stores the project's tombstone in `project_tombstones` before it sweeps. Its `laid_at` is the `occurred_at` of the deletion. The tombstone is stored when the deletion message arrives, not on the sweep's continuation messages. The chore that deletes idea tombstones deletes these too.

Capturing checks for the tombstone inside its transaction, and refuses if there is one. Cataloguing does the same. This matters because the sweep reads the catalog. So an idea that was captured before the deletion, but not yet catalogued, is discarded when it is catalogued, instead of being listed.

On PostgreSQL, a capture or a cataloguing may check for the tombstone while the tombstone is being stored. Then one of the two loses the race and is tried again. So the sweep that follows finds what was written. (*The lock was removed on 2026-11-04, when every PostgreSQL transaction became serializable.* Before that, there was a lock on the project.)

**Core owns each write's transaction.** Capturing, retitling and discarding each begin a transaction in `IdeasCore`, after it has asked for permission. `IdeaService` carries out the command in that transaction, and hands it to the event-sourcing service as `within`. `transacting::retrying!` tries a change again after a lost race: up to five times if the change names no version, and once if it names one (`eventsourcing::tries_for(expected)`). So that conflict is answered with a 412.

Cataloguing stores or removes summaries in the transaction of its listener, `catalogue-idea`. This listener receives its messages through [`inbox::Inboxed`](../../libraries/inbox/README.md). The inbox begins the transaction and records in it that the message was received. So a message delivered twice is catalogued only once. A message whose cataloguing failed is taken in again on its next delivery. The catalog's writes use this transaction.

Deleting old tombstones runs in a transaction of its own. `forget-old-idea-hearings` deletes old records of received messages. The sweep discards each idea in a transaction of its own, on purpose. So the sweep is a plain `messaging::Handler` that does not use the inbox. Its steps are idempotent, and each of its continuation messages is derived from the message it follows on.

The catalog's suite proves that a remember, a forget, and a deletion of tombstones leave nothing behind if their transaction is dropped.

**One way in: `IdeasCore`.** The feature's core is entered through one struct. Its methods are everything the ideas feature does:

- what someone does: listing, capturing, reading, retitling, discarding, following,
- and what the feature does itself: cataloguing, the sweep after a project is deleted, and finding an idea's project for the broadcast.

Every permission is asked there and nowhere else. The core hands on to `IdeaService`. `IdeaService` only carries out the commands and records who acted, and it cannot be reached from outside the core. A write answers with the idea as written. Its errors are one enum, `IdeasError`. The adapters only translate.

**Ideas are there only for those in their project.** Listing a project's ideas, reading one and following them live ask the [`Authorizer`](../../libraries/authorizing) for `Read`. Capturing, retitling and discarding ask for `Write`, which an author has. An idea knows its project. So the core reads the idea first and asks about its project, before it judges anything else in the request. For example, a stranger who sends a title that is too long is told that the idea is not there, not that the title is too long. Someone who may not read the project gets a 404, the same as for an idea or a project that is not there. Someone who may read but not write gets 403 `not-an-author`.

**Who may do what is kept by the feature itself,** through [`access`](../../libraries/access/README.md). This data is stored in the ideas' own database. Two listeners receive their messages through the inbox:

- `ideas-remember-roles` receives the members' events (`founded`, `joined`, `role.changed`, `left`, `removed`, `disbanded`).
- `ideas-remember-trash` receives when projects are put in the trash and restored.

`forget-old-idea-tombstones` deletes their old tombstones, together with the feature's own. If there are accounts, the core asks `KeptAccess`. After a change to a project's access is committed, `KeptAccess` tells everyone who follows that project live. Without accounts, everyone is allowed.

**Crates:** `core` (`IdeasCore`, aggregate, catalog port, service) · `contract` · `adapters/store` (stored-event codec) · `adapters/catalog` (listings per project) · `adapters/messaging` (event publisher, catalog projector and who-may-do-what listeners) · `adapters/rest` · `wiring` · `tests`.
