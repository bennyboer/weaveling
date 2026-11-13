# projects

A project is the book an author is working on — the thing everything else hangs off.

It holds almost nothing: a name, when it began, and when it was last touched. What it is *for* is identity and deletion. Every idea, board, outline and scene names a project, and deleting one has to leave nothing behind.

**`Project` is an aggregate** — `Started`, `Renamed`, `Deleted` — event-sourced like every other feature since [M11a](../../ROADMAP.md#step-1--projects-becomes-an-aggregate--done). Deletion is why: it is exactly where an author wants history, and `project.deleted` on the wire is what the cascade listens for. A deleted project is gone from the listing and refuses further changes, but its stream stays and still says what it was called and who ended it.

Unusually, the aggregate carries its own `created_at` and `updated_at`, because the project list shows an author when they last touched each one. They come from each event's `occurred_at` — never a clock the aggregate reaches for — and travel inside the snapshot body, since a snapshot is written long after the fact.

**Each sees only their own projects.** For the list, the core asks the [`Authorizer`](../../libraries/authorizing) once which projects the person asking may read. In local mode the answer is `Everything`. On a server it is `Only` the projects the person is in. The catalog then returns the projects `among` those, newest first.

The members feature says who is in which project. The projects feature stores this itself, through [`access`](../../libraries/access/README.md):

- `projects-remember-roles` receives the members' events.
- `projects-remember-trash` receives when the projects themselves are put in the trash and restored.
- If there are accounts, the `KeptAccess` built on this data is the projects' `Authorizer`. Without accounts, everyone is allowed.
- A project's founder can use the project once the founding message has been received. That is a few milliseconds after the start.
- `forget-old-project-tombstones` deletes what ended, together with the feature's own tombstones.

**The channel tells each their own.** The topic `projects` on the one socket, `/api/live`, with `mine`, is filtered per follower. A change to a project is sent to a follower only if that follower may read the project. The `Authorizer` is asked for every frame. With `project`, a single project is followed. Only someone who may read it can follow it. For anyone else it is `not-there`. Being let into a project or out of it is announced by the members, on [their own channel](../members).

**A project is there only for those in it.** Reading a project asks the `Authorizer` for `Read`. Every command after the start asks for `Manage`, which only an owner has: renaming the project, changing its details, and deleting it. Someone who may not even read the project gets the same 404 as for a project that is not there, built the same way. So strangers cannot learn whether a project exists. Someone who may read it but not manage it gets 403 `not-an-owner`, because there is nothing to hide from them. Starting a project asks nobody. The answer to a start reads the new project without asking, because its owner is not yet listed among its members.

**Deleting puts a project in the trash, for thirty days.** *Slice 6b, decided on 2026-11-02: the user planned the trash this way.*

- `DELETE /projects/{id}` calls `ProjectsCore::trash`. In one transaction, this appends `Trashed { until }` and puts the project into the feature's [trash](../../libraries/trash/README.md) with `put_into(transaction, "project", id, until)`. `until` is now plus `KEPT_IN_THE_TRASH_FOR`, which is thirty days. Because it is one transaction, neither outlives the other. Trashing a project again is not a failure, and it does not extend the time.
- In the trash, a project is gone: it is not listed, cannot be read, and refuses every change with `project-trashed`. But its owners find it in `GET /projects/trash`. Each entry is a `TrashedProjectDTO` that says until when the project is kept.
- Any owner can restore it with `POST /projects/{id}/restore`. This appends `Restored` and calls `take_out`, in one transaction. If the project was not in the trash, the answer is `project-not-trashed`.
- Only owners can trash, restore and see the trash, just as only owners could delete before.

**The purge is today's deletion, thirty days late:**

- The chore `purge-the-projects-trash` is `PurgeTheTrash` with `ProjectsPurge`. It hands each project whose time is over to `ProjectsCore::purge`.
- `ProjectsCore::purge` appends `Deleted` as the system and takes the project out of the trash, in one transaction. So `project.deleted` is sent as before, and every feature that receives it sweeps as before.
- If a project was restored in the meantime, it is left alone. Only its trash entry is removed.
- `Delete` is refused for a project that is not in the trash. So a project is only deleted by its purge, or when nobody is left in it.

**A project nobody is left in is deleted.** *Slice 9d-3b, 2026-11-10:* the listener `delete-abandoned-project` receives `members.abandoned`. The members send this once the account of the last member is deleted. The listener has `ProjectsCore::delete_abandoned` append `Deleted` as the system. `DeleteAbandoned` ignores the trash. In the same transaction, the project is taken out of the trash. A project that is already deleted is not a failure. `project.deleted` is then sent, just as after a purge.

The list stores `trashed_until` next to each summary. It leaves the projects in the trash out of `all` and `among`. `in_the_trash` returns them, the ones purged soonest first. `project.trashed` and `project.restored` are sent on the wire and on the live topics, like every other change.

**One way in: `ProjectsCore`.** Everything the projects feature does goes through one struct, and every permission is asked there:

- Listing asks which projects the agent may read.
- Reading and following ask for `Read`.
- Renaming, describing and deleting ask for `Manage`.
- Starting asks nothing, because anyone signed in may start a project. It answers with the project as started, because its starter becomes its owner only once the members have received the start.

A write answers with what it wrote, so nothing has to read a project back without asking. `ProjectService` only carries out the commands and records who acted, and nothing outside the core can reach it. Errors are one enum, `ProjectsError`.

**A project forgotten stays forgotten.** The catalog removes a deleted project in one transaction: it stores a tombstone in `project_tombstones` and deletes the project's summary. `remember` refuses a project with a tombstone. It also replaces a summary only with a newer version. So a summary that arrives late cannot bring the project back.

On PostgreSQL, a `remember` may read no tombstone just before a forget commits one. Then the `remember` loses the race, because both transactions are serializable. It is tried again and finds the tombstone. (*The lock was removed on 2026-11-04, when every PostgreSQL transaction became serializable.* Before that, there was a lock on the project.) On SQLite, only one writer at a time keeps them apart.

The chore `forget-old-project-tombstones` deletes the tombstones one day after a change about the project can no longer be retried. Every feature handles its tombstones this way.

**Core owns each write's transaction.** Starting, renaming, changing details and deleting each begin a transaction in `ProjectsCore`, after it has asked for permission. `ProjectService` carries out the command in that transaction, and hands it to the event-sourcing service as `within`. `transacting::retrying!` tries a change again after a lost race: up to five times if the change names no version, and once if it names one (`eventsourcing::tries_for(expected)`). So that conflict is answered with a 412.

Cataloguing stores or removes summaries in the transaction of its listener, `catalogue-project`. This listener receives its messages through [`inbox::Inboxed`](../../libraries/inbox/README.md). The inbox begins the transaction and records in it that the message was received. So a message delivered twice is catalogued only once. A message whose cataloguing failed is taken in again on its next delivery. The catalog's writes use this transaction.

Deleting old tombstones runs in a transaction of its own. `forget-old-project-hearings` deletes old records of received messages. The catalog's suite proves that a remember, a forget, and a deletion of tombstones leave nothing behind if their transaction is dropped.

**Crates:** `core` (`ProjectsCore`, aggregate, catalog port, service) · `contract` (DTOs and routing keys, shared with the client) · `adapters/store` (stored-event codec) · `adapters/catalog` (in memory, PostgreSQL and SQLite listings) · `adapters/messaging` (event publisher and catalog projector) · `adapters/rest` · `wiring` · `tests`.
