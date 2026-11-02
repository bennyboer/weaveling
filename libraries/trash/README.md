# trash

What is marked for removal, restorable until a deadline, and purged for good by a chore once it has passed. The mechanism is shared; the domain is not: each feature keeps its own state and events, a project trashed and restored, and its own way back.

**An entry is a kind, an id and until when it is kept**, `kept_until`, the moment from which it is due to be purged, *named so 2026-11-02, the user asking whether `swept_at` meant when it was put in or taken out: it is neither, and a purge comes at the first round after it.* A feature keeps its trash in its own database, as it keeps its inbox: `InMemoryTrash`, `PostgresTrash` behind the `postgres` feature and `SqliteTrash` behind `sqlite`, each laying its table down with `migrations()`, tracked in its own ledger, `_sqlx_migrations_trash`. The kind keeps apart the things one feature trashes, as `"project"` beside a later `"account"`.

**Every write takes the transaction.** `put_into(transaction, kind, id, until)` puts a thing in the trash, its entry kept until then, or keeps it until another moment if it is there already; `take_out(transaction, kind, id)` takes it out, answering whether it was there. So a feature trashes with the event that says so, and restores with the one that undoes it, in one [transaction](../transacting/README.md), and neither outlives the other. `due(at, at_most)` answers everything of every kind to be purged by then, oldest first, each a **`Trashed`**, its kind and its id, from the pool, *its kind told rather than asked for, the user choosing so 2026-11-02, so one chore purges all a feature trashes.*

```rust
let mut transaction = transactions.begin().await?;
projects.trashed(&mut transaction, &project, agent).await?;
trash.put_into(&mut transaction, "project", project.as_str(), now + KEPT_FOR).await?;
transaction.finish().await?;
```

**How long a thing stays is the feature's to say**, where it is wired: the library knows no period, only until when each entry is kept.

**`PurgeTheTrash` is the chore that purges**, one a feature, *called purging, not sweeping, the user choosing the word 2026-11-02, which also keeps it apart from the sweeps that follow a project's deletion.* Named as the feature wires it, it comes round hourly, asks what is due, `PURGED_AT_MOST` at a time, and hands each `Trashed` to the feature's **`Purge`**, until a batch comes back short. A purge removes the thing for good, by its kind, and takes it out of the trash in one transaction of its own, so a purge done is never done again, and a thing restored meanwhile has nothing left to purge. One that fails is logged and stays for the next round, the others purged still, and the chore says it failed. A purge that answers done but leaves its entry behind would be handed it again for ever, so the chore stops the round and says so instead.

**Tests.** `suite.rs` holds the conformance cases every backend runs: due once its time has come and not before, each with its kind and one id of two kinds two things, oldest first and no more than asked for, taken out once, put in again to keep it until later, and nothing put in or taken out in a transaction dropped. `purging.rs` holds the chore's: every due entry of every kind purged batch after batch, handed over with its kind, and nothing before its time, one refused left for the next round, and a purge that leaves its entry stopping the round.
