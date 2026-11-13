# appearances

The answer to *where does this appear?* — for an idea today: in which sections of the outline, and in which scenes.

**It is a read model, and owns nothing.** No view writes here. Each view says what it holds — the outline notes an idea in a section, a scene links one — and announces it; this feature listens and keeps the answer in one place. It is a projection, so it is rebuildable from those announcements and nothing else, and a new view joins by announcing rather than by anyone asking it. That is the point of keeping it out of [`ideas`](../ideas): an idea must not change when a view is added. See [*"Where does this idea appear" is a read model*](../../ARCHITECTURE.md#ideas-and-scenes-are-two-pools--decided).

**Only places in the book count.** A section and a scene say where an idea is *used*. The board is where ideas are *thought about*, and with one board per project, *this idea is on the board* says almost nothing — so boards are deliberately not places here. They can join later as one more `Place` variant, without a migration, if several boards ever make *find it on board X* worth answering.

**A `Subject` appears at a `Place`.** Today the only subject is an idea. Characters, moments on the timeline, even a scene asking *which sections hold me* have the same shape, so the storage says `subject_type` / `subject_id` and `place_type` / `place_id` and never the word *idea*. The domain stays typed, though: `Subject` and `Place` are enums, and adding a variant means adding its listeners too — with the compiler pointing at every place that has to decide what the new kind may appear in. That is the guard against [a reference-to-anything turning to soup](../../ARCHITECTURE.md#ideas-and-scenes-are-two-pools--decided): which pairs exist is said in code, not left open in a table.

**A place names, it does not copy.** `Section(id)` and `Scene(id)` — ids only. A section's title and a scene's title are looked up by whoever shows them, the same [frontend join](../../ARCHITECTURE.md#the-board-renders-through-a-frontend-join) the board uses for card titles; storing them here would mean chasing every rename. And the type is part of a place's identity: a section and a scene whose ids happen to match are two places.

**A move is a forget and a remember.** The outline announces a note moving between sections as `Detached` then `Attached`, never as a bare `Attached` that would leave the old section unnamed — so this feature needs no idea of where a note *was*.

**Forgetting comes in the shapes the views announce.** One appearance (a detach, an unlink), every appearance of a subject (an idea was discarded), and every subject at a place (a section removed, a scene deleted).

**A late message never undoes a newer one.**
A refused message can be retried long after the messages that followed it were handled.
So every appearance stores the `version` it was last told about, and whether it is there.
Remembering and forgetting a single appearance only take effect if they are not older than that version.
A forgotten appearance stays as a row marked absent.
So a late *attached* still meets the *detached* that came after it.
The version is the outline's, from the envelope, or the scene's own, from the link message.
Within one section or one scene, it orders every note.
A discarded idea, a removed section and a deleted scene are instead stored as **gone for good**.
That is because the message that forgot them came from a different stream, whose versions say nothing about the outline's.
And none of them can come back.
A version equal to the stored one also takes effect: that is the same message delivered again.

**An appearance and a tombstone never cross.**
On PostgreSQL, a mark may read no tombstone before a forget commits one.
That mark then loses the race and is tried again.
(*The lock was removed on 2026-11-04, when every PostgreSQL transaction became serializable.* Before that, locks on the place and the idea did this.)
On SQLite, only one writer runs at a time, which has the same effect.
Processing one message marks at one place.
Two messages that mark the same ideas at once can conflict.
PostgreSQL then refuses one of them, and it is tried again.

**Gone for good lasts as long as a late message can.**
Each tombstone in `subject_tombstones` and `place_tombstones` stores `laid_at`: when the thing it marks went.
That is the `occurred_at` of the message saying so. The first such message sets it.
The chore `forget-old-appearance-tombstones`, a [`ForgetOldTombstones`](../../libraries/chores), runs hourly.
It forgets the tombstones laid more than `TOMBSTONES_KEPT_FOR` ago.
That is a day longer than a refused message may be retried.
So no message that could still bring one back is left.
A discarded idea's project is forgotten together with its tombstone.
The rows marked absent stay.
There is one of them per idea and place that ever met, and they go when the idea or the place goes.

**A discarded outline is not followed.**
Its announcement does not list its sections.
Instead, its sections go with its project, when the project is deleted.
That is the only way an outline is discarded.

**An idea's project is learnt from the ideas alone.**
The idea's `CAPTURED` names its project.
`remember-project-of-idea` stores it (`remember_project`), apart from the appearances, because it answers a different question.
The outline's attachments and the scenes' links name *their* project in their [context](../../libraries/eventsourcing). That is the place's project.
It is stored beside the place (`remember_place_project`) until the place is forgotten.
A place that was already forgotten stores no project, however late its link arrives.
An idea appears only in places of its own project.
A link from another project, made with an idea's id known from elsewhere, is stored like any other, but shown nowhere.
It does not move the idea into another project, as it did when the links named the idea's project (*13c-1, 2026-11-11*).
It does not matter whether `CAPTURED` or a link arrives first, because the two are only matched when someone asks.

`forget-appearances-of-deleted-project` receives the deletion of a project.
It removes every place of that project (`forget_places_of_project`), using the index on `place_projects (project)`.
Each place gets its tombstone, as a removed section or a deleted scene would.
Each place loses its appearances and its project.
So nothing of the project stays here once an account and its projects are gone.

A discarded idea's project is kept as long as the idea's tombstone in `subject_tombstones`.
That is because whoever followed the idea is told that it now appears nowhere.
Only those who may read its project may receive that frame.
The project and the tombstone are forgotten together.

Where an idea appears is told only to those who may read its project: `Read` is asked of the [`Authorizer`](../../libraries/authorizing).
`GET /api/appearances?idea=` answers 404 to anyone else, as if there were nothing.
An idea whose project is not known yet appears nowhere, because there is nothing to tell.
The topic `appearances` on the one socket, `/api/live`, with an `idea`, asks when subscribing and again for every frame.
So someone who followed an idea before its project was known receives nothing about it, unless they may read it.
Deleting a project could become a single delete by project here. That waits until it is needed.

**Who it hears.**

- `index-outline-appearances` receives `ATTACHED` / `DETACHED` of ideas, and `SECTION_REMOVED`. A scene attached to a section is the book itself, not an appearance.
- `index-scene-appearances` receives `scene.idea.linked`, `scene.idea.unlinked` and `scene.deleted`.
- `forget-appearances-of-discarded-idea` receives the idea's `DISCARDED`.
- `remember-project-of-idea` receives the idea's `CAPTURED`.

All are kept deliveries.
Each is a `KeptHandler` run through [`inbox::Inboxed`](../../libraries/inbox/README.md).
It begins the transaction, skips a message the feature's inbox has received before, and hands the transaction on.
`appeared`, `disappeared`, `place_gone` and `idea_discarded` take it as `within` and join it.
So an appearance and the record of having received the message commit together.
They also stay idempotent — remembering twice is one row, forgetting what is gone is nothing — for messages that arrive out of order.
The chore `forget-old-appearance-hearings` forgets the old records of received messages.

**Who may read a project is kept by the feature itself,**
through [`access`](../../libraries/access/README.md), stored in the appearances' own database.
`appearances-remember-roles` receives the members' messages (`founded`, `joined`, `role.changed`, `left`, `removed`, `disbanded`).
`appearances-remember-trash` receives the messages about projects put in the trash and restored.
Both run through the inbox.
The chore `forget-old-appearance-tombstones` forgets their tombstones, beside the feature's own.
Where there are accounts, the core asks `KeptAccess`. Without accounts, it allows everyone.

**Route:** `GET /api/appearances?idea={id}` → `[{ "type": "scene" | "section", "id" }]`, in one stable order.

**One way in: `AppearancesCore`.**
The REST route, the live route and its filter, the three listeners and the chore all go through one struct.
It asks the `Authorizer` about the idea's project: where an idea appears is told only to those who may read its project.
An idea whose project is not known yet may be followed, and returns nothing.
But a change about it is told to nobody until its project is known.
Errors are one enum, `AppearancesError`.

**Core owns each write's transaction.**
The feature stores no events.
Still, `Ports` carries its own `Transactions` on its database.
`AppearancesCore` begins one for every write, or joins the listener's transaction.
An appearance and the project of its place are stored in one transaction. Before, they were two writes.
The catalog's writes take the transaction.
`TellingAppearances` reads where an idea appears and reports it in an `after_commit`.
So what it reports is what was committed, never a write that was rolled back.
Nothing is retried, because no write here aims at a version.
The catalog's own guards handle messages that arrive out of order.

**Crates:** `core` (`AppearancesCore`, `Subject`, `Place`, the catalog port) · `contract` (`PlaceDTO`) · `adapters/catalog` (in memory, PostgreSQL and SQLite, one conformance suite for all three) · `adapters/messaging` (the three listeners) · `adapters/rest` · `wiring` · `tests`.
