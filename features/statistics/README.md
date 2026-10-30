# statistics

How many words each scene holds, kept where a total can be had without reading every scene.

It owns no aggregate and takes no command. It is a read model only: **`keep-word-counts`** hears `scene.words.counted`, which the scenes feature publishes when a scene's text settles, and remembers the count for that scene; it hears a scene deleted and forgets it. `GET /statistics/word-counts?project=…` answers every scene's count in a project, which the writing view adds up for a section and for the book, and the topic `word-counts` on the one socket, `/api/live`, tells each new count to whoever follows the project, so the totals move as other authors write.

**A feature of its own rather than a column in the scene catalog,** since a feature may not call another, and counting is a fact about text that the scenes feature already knows how to tell. Keeping it here leaves room for what else an author may want counted, words per day or per session, without the scenes feature learning about it.

**A project's counts are there only for those in it.** Asking a project's counts and following them live ask the [`Authorizer`](../../libraries/authorizing) for `Read`; whoever may not read the project is answered 404, as for one not there, and the live route too, before it upgrades. The feature takes no command, so nothing asks `Write`.

**A deleted scene stays gone.** Its count is forgotten and a tombstone kept in `scene_tombstones`, so a count that arrives late, or is retried, does not bring it back; a count is kept only if it is not older than the one held, by when it was counted. The tombstone keeps `laid_at`, when the scene was deleted, the `occurred_at` of the message saying so, and the chore `forget-old-statistics-tombstones`, a [`ForgetOldTombstones`](../../libraries/chores), forgets it once no message about the scene can still arrive, in batches.

The count itself is [`words::words_in`](../../libraries/words), the same rule the browser uses as an author types, so the two never disagree.

**One way in: `StatisticsCore`.** The word counts, their live route, the listener keeping them and the chore all go through one struct, which asks `Read` of the project. Errors are one enum, `StatisticsError`.

**Crates:** `core` (`StatisticsCore`, word count, catalog port, links to projects and scenes) · `contract` · `adapters/catalog` (in memory, PostgreSQL and SQLite) · `adapters/messaging` (the listener) · `adapters/rest` · `wiring` · `tests`.
