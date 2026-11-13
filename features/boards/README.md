# boards

The spatial view: ideas pinned on a canvas, where an author can see them all at once.

A board holds **placements** — an idea, a spot, a size — and nothing else about the idea. The title an author reads on a card comes from the [idea catalog](../ideas), joined in the client. That is deliberate: a board that stored titles would hold a second copy of a fact it does not own, and every retitle would have to chase it. See [the frontend join](../../ARCHITECTURE.md#the-board-renders-through-a-frontend-join).

**`Board` is an aggregate** — `Started`, `IdeaPinned`, `IdeaMoved`, `IdeaResized`, `IdeaRaised`, `IdeaUnpinned`. Moving and resizing are separate events even though one `Reshape` command can do both, because an event says what changed rather than what a caller asked for. Multiple boards per project are in the model from the first event even though the first version ships one, because keying placements by project id is the shortcut that would turn a second board into a migration of everything ever written.

**A project cannot start its own board**, since a feature may not call another feature. So a board is started on first open — find-or-start, under one write guard — and a project may legitimately have no board yet. Anything sweeping a project has to tolerate that.

**Two projectors and two listeners.**

- `catalogue-board` keeps the board summaries.
- `index-pinned-ideas` keeps the index of which boards hold a given idea.
- `unpin-discarded-idea` receives `idea.discarded` and stores a tombstone for the idea.
- `let-go-of-discarded-idea-everywhere` and `let-go-of-gone-ideas` take a gone idea off the boards. Each of them is triggered by one of the two listeners above.

**Heard once, or safe to hear again.** `catalogue-board` receives its messages through [`inbox::Inboxed`](../../libraries/inbox/README.md). The board's summary and the record that the message was received are committed in one transaction. The chore `forget-old-board-hearings` deletes these records once the tombstones' time is over. `index-pinned-ideas`, `unpin-discarded-idea`, `let-go-of-discarded-idea-everywhere` and `let-go-of-gone-ideas` also use the inbox. Only the sweep `discard-boards-of-deleted-project` stays a plain listener.

**A deleted project opens no board.** `boards-remember-deleted-projects` receives the deletion through the inbox. It stores the project's tombstone in the catalog, in `project_tombstones`. In the same transaction, it asks for the sweep with `follow-up.board.sweep-project`. `discard-boards-of-deleted-project` receives this follow-up instead of `project.deleted`. So it reads the registry only after the tombstone is committed.

`open` refuses a project with a tombstone, as if the project were not there. After its own claim on the board is committed, `open` checks again. If a tombstone was stored in the meantime, it discards the board it started. So when an open and a deletion race, either the open sees the tombstone, or the sweep sees the board. The sweep does nothing if the board is not there or is already discarded. The chore that deletes the other tombstones also deletes the projects' tombstones.

**A pin racing its idea's discard is still let go of, by a follow-up.** Two listeners may run at the same time: one writes the index of a board's pins, the other writes the idea's tombstone. Each must see what the other committed. If each read in its own transaction, either could miss the other. So each one writes, and in the same transaction puts a follow-up in the outbox. The outbox sends the follow-up only after that transaction is committed:

- `unpin-discarded-idea` sends `follow-up.board.let-go-everywhere` to `let-go-of-discarded-idea-everywhere`. That listener unpins the idea from every board that holds it, according to the index.
- `index-pinned-ideas` sends `follow-up.board.let-go-gone` to `let-go-of-gone-ideas`. That listener unpins the first idea on the board that has a tombstone.

When two writers race, the follow-up of the later one reads what the earlier one committed.

The follow-ups' routing keys begin with `follow-up.`, so no `board.#` subscription receives them. They are defined in the messaging adapter, not in the contract. Each one lives in the file of the listener that sends it and the listener that receives it, next to the function that builds it. So a listener that sends one reads as: first write, then `outbox.enqueue_within` of that message.

A board removes only one gone idea per message, because two unpins on one board in one transaction would collide. The unpin creates its own `board.idea.unpinned`. The index receives it and asks the board again, until no gone idea is left. Every step is one transaction, the one of the listener handling the message. Each step is idempotent: an idea that is not pinned, or a board that is discarded, is not a failure.

**A board is there only for those in its project.** Opening a project's board, reading it and following it live ask the [`Authorizer`](../../libraries/authorizing) for `Read`. Pinning, moving, resizing and unpinning ask for `Write`, which an author has. The service reads the board first and then asks about its project. Someone who may not read the project gets a 404, the same as for a board that is not there. The live route does the same, before it upgrades the connection. Someone who may read but not write gets 403 `not-an-author`. A reader who opens a project's board before anyone else has done so starts it, as opening always did. What the feature does itself, as `Agent::System`, is allowed by the authorizer.

**Who may do what is kept by the feature itself,** through [`access`](../../libraries/access/README.md). This data is stored in the boards' own database. Two listeners receive their messages through the inbox:

- `boards-remember-roles` receives the members' events (`founded`, `joined`, `role.changed`, `left`, `removed`, `disbanded`).
- `boards-remember-trash` receives when projects are put in the trash and restored.

`forget-old-board-tombstones` deletes their old tombstones, together with the feature's own. If there are accounts, the core asks `KeptAccess`. After a change to a project's access is committed, `KeptAccess` tells everyone who follows a board of that project live. Without accounts, everyone is allowed.

**A discarded idea leaves a tombstone.** `idea_tombstones` stores every idea that was discarded. So a pin that is indexed after the discard is still removed. The tombstone stores `laid_at`, which is the `occurred_at` of the discard. The chore `forget-old-board-tombstones`, a [`ForgetOldTombstones`](../../libraries/chores), deletes the tombstone in batches once no message about the idea can arrive anymore. After that, a pin of an idea discarded that long ago, from an outdated or forged request, is no longer removed. The board keeps it. The client does not show it, because it only draws ideas it knows.

**A board pins only ideas of its own project.** `boards-remember-projects-of-ideas` receives `idea.captured` and stores the idea's project in the [`belonging`](../../libraries/belonging/README.md) store. `unpin-discarded-idea` removes that entry when it stores the tombstone. So nothing of a deleted project's ideas remains. `pin` refuses an idea that is known to belong to another project with 404 `not-found`. For this board, the idea is indeed not there, and the answer tells nothing about it. An idea the board has not heard of yet is pinned. This is because a board may pin an idea that was captured a moment ago, before the capture message reaches the board (*13c-2b, 2026-11-11*).

**A board forgotten stays forgotten.** The catalog removes a discarded board in one transaction: it stores a tombstone in `board_tombstones` and deletes the board's summary. `remember` refuses a board with a tombstone, so a summary that arrives late cannot bring the board back.

On PostgreSQL, a `remember` may read no tombstone just before a forget commits one. Then the `remember` loses the race, because both transactions are serializable. It is tried again and finds the tombstone. (*The lock was removed on 2026-11-04, when every PostgreSQL transaction became serializable.* Before that, there was a lock on the board.) On SQLite, only one writer at a time keeps them apart.

`forget-old-board-tombstones` deletes these tombstones together with the idea tombstones. A summary holds only the board and its project, and the project never changes. So it needs no version guard.

**Core owns each write's transaction.** `BoardsCore` begins the transaction of every write after it has asked for permission. A change runs through `transacting::retrying!`. After a lost race, a change that names no version is tried up to five times, and a change that names a version is tried once (`eventsourcing::tries_for(expected)`). So that conflict is answered with a 412. The system's own changes name no version. Opening claims the board and starts it in one transaction, so a claim never points at nothing. Cataloguing, recording what a board holds, storing a tombstone and deleting old tombstones each begin their own transaction, and the catalog's writes use it.

**One way in: `BoardsCore`.** Everything goes through this one struct:

- opening, reading, following, pinning, reshaping and unpinning,
- and what the feature does itself: cataloguing, recording which board holds which idea, removing a discarded idea, and the sweep after a project is deleted.

It asks for `Read` or `Write` on the board's project. The system's own work is done, and recorded, as `Agent::System`. Only the sweep discards a board. A write answers with the board as written. `BoardService` only carries out the commands, in the transaction that `BoardsCore` hands it, and nothing outside the core can reach it. Errors are one enum, `BoardsError`.

**Crates:** `core` (`BoardsCore`, aggregate, catalog port, service, `Spot`/`Size`) · `contract` · `adapters/store` · `adapters/catalog` · `adapters/messaging` (event publisher, catalog projector, discard listener, who-may-do-what listeners) · `adapters/rest` · `wiring` · `tests`.
