# statistics

This feature stores how many words each scene holds. So a total can be calculated without reading every scene.

It owns no aggregate and takes no command. It is a read model only:

- **`keep-word-counts`** receives `scene.words.counted`. The scenes feature publishes this message when a scene's text has settled. The listener stores the count for that scene. When it receives that a scene was deleted, it deletes the scene's count.
- `GET /statistics/word-counts?project=…` returns the count of every scene in a project. The writing view adds them up for a section and for the whole book.
- The topic `word-counts` on the one socket, `/api/live`, sends each new count to everyone who follows the project. So the totals change while other authors write.

**A feature of its own rather than a column in the scene catalog,** because a feature may not call another feature. Counting is a fact about text, and the scenes feature already knows how to announce it. Keeping it here leaves room for other things an author may want counted, such as words per day or per session, without the scenes feature having to know about them.

**A project's counts are there only for those in it.** Asking for a project's counts and following them live ask the [`Authorizer`](../../libraries/authorizing) for `Read`. Someone who may not read the project gets a 404, the same as for a project that is not there. The live route does the same, before it upgrades the connection. The feature takes no command, so nothing asks for `Write`.

**Who may read a project is kept by the feature itself,** through [`access`](../../libraries/access/README.md). This data is stored in the statistics' own database. Two listeners receive their messages through the inbox:

- `statistics-remember-roles` receives the members' events (`founded`, `joined`, `role.changed`, `left`, `removed`, `disbanded`).
- `statistics-remember-trash` receives when projects are put in the trash and restored.

The chore `forget-old-statistics-tombstones` deletes their old tombstones, together with the feature's own. If there are accounts, the core asks `KeptAccess`. If a follower leaves the project, its live connection is closed once that change is committed. Without accounts, everyone is allowed.

**A deleted scene stays gone.** Its count is deleted, and a tombstone is stored in `scene_tombstones`. So a count that arrives late, or is retried, does not bring the scene back. On PostgreSQL, storing a count may read no tombstone just before the deletion commits one. Then storing the count loses the race and is tried again. (*The lock was removed on 2026-11-04, when every PostgreSQL transaction became serializable.* Before that, there was a lock on the scene.) On SQLite, only one writer at a time keeps them apart. A count is only stored if it is not older than the stored one, judged by when it was counted.

The tombstone stores `laid_at`: when the scene was deleted, which is the `occurred_at` of the message about the deletion. The chore `forget-old-statistics-tombstones`, a [`ForgetOldTombstones`](../../libraries/chores), deletes the tombstone in batches once no message about the scene can arrive anymore.

The count itself is [`words::words_in`](../../libraries/words), the same rule the browser uses as an author types, so the two never disagree.

**One way in: `StatisticsCore`.** The word counts, their live route, the listener that stores them, and the chore all go through one struct. It asks for `Read` on the project. Errors are one enum, `StatisticsError`.

**Core owns each write's transaction.** The feature stores no events. Still, `Ports` has its own `Transactions` on the feature's database, and `StatisticsCore` begins a transaction for every write: storing a count, storing a scene's tombstone, and deleting old tombstones. The catalog's writes use it. `TellingWordCounts` sends a count in an `after_commit`, so never for a write that was rolled back. Nothing is retried, because no write here names a version. The catalog's own guards handle counts that arrive out of order.

**Heard once.** `keep-word-counts` is a `KeptHandler` that receives its messages through [`inbox::Inboxed`](../../libraries/inbox/README.md). The inbox begins the transaction, checks whether the feature's inbox has received the message before, and hands the transaction on. `counted` and `scene_deleted` take it as `within` and join it. So a count and the record that its message was received are committed together. The chore `forget-old-statistics-hearings` deletes old records of received messages, at the same time as tombstones are deleted.

**Crates:** `core` (`StatisticsCore`, word count, catalog port, links to projects and scenes) · `contract` · `adapters/catalog` (in memory, PostgreSQL and SQLite) · `adapters/messaging` (the listener) · `adapters/rest` · `wiring` · `tests`.
