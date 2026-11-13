# trash

Things that are marked for removal. They can be restored until a deadline. Once the deadline has passed, a chore purges them for good. The mechanism is shared, but the domain is not. Each feature keeps its own state and events (for example, a project that is trashed and restored), and its own way of restoring.

**An entry is a kind, an id, and a time until which it is kept:** `kept_until`. That is the moment from which the entry is due to be purged. *The name was chosen on 2026-11-02. The user asked whether the old name `swept_at` meant when the entry was put in or taken out. It meant neither. A purge happens at the first chore run after that moment.*

A feature keeps its trash in its own database, as it does with its inbox. There are three implementations: `InMemoryTrash`, `PostgresTrash` behind the `postgres` feature, and `SqliteTrash` behind `sqlite`. Each creates its table with `migrations()`, tracked in its own ledger, `_sqlx_migrations_trash`. The kind keeps apart the different things one feature trashes, such as `"project"` and, later, `"account"`.

**Every write takes the transaction.**

- `put_into(transaction, kind, id, until)` puts a thing in the trash, and keeps its entry until `until`. If the thing is already there, it changes the time it is kept until.
- `take_out(transaction, kind, id)` takes it out, and returns whether it was there.

So a feature trashes a thing together with the event that says so, in one [transaction](../transacting/README.md). It restores a thing together with the event that undoes it, also in one transaction. Neither one outlives the other.

`due(at, at_most)` returns everything of every kind that is due to be purged by then, oldest first. Each result is a **`Trashed`**, with its kind and its id. It reads from the pool. *The result includes the kind, instead of the caller asking for one kind. Decided on 2026-11-02: the user chose this, so that one chore purges everything a feature trashes.*

```rust
let mut transaction = transactions.begin().await?;
projects.trashed(&mut transaction, &project, agent).await?;
trash.put_into(&mut transaction, "project", project.as_str(), now + KEPT_FOR).await?;
transaction.finish().await?;
```

**How long a thing stays is decided by the feature,** where it is wired. The library knows no period. It only knows until when each entry is kept.

**`PurgeTheTrash` is the chore that purges.** There is one per feature. *It is called purging, not sweeping. Decided on 2026-11-02: the user chose the word. It also keeps it apart from the sweeps that follow a project's deletion.*

It is named the way the feature wires it. It runs hourly. It asks what is due, `PURGED_AT_MOST` at a time, and hands each `Trashed` to the feature's **`Purge`**. It continues until a batch comes back with fewer than that.

A purge removes the thing for good, according to its kind. In the same transaction, which is its own, it takes the thing out of the trash. So a purge that is done is never done again. And a thing that was restored in the meantime has nothing left to purge.

If a purge fails, this is logged, and the entry stays for the next run. The other entries are still purged, and the chore reports that it failed. A purge might report success but leave its entry behind. Then it would be handed the same entry again forever. So in that case the chore stops the run and reports it instead.

**Tests.** `suite.rs` holds the conformance cases that every backend runs:

- an entry is due once its time has come, and not before,
- each entry comes with its kind, and one id of two kinds is two separate things,
- entries come oldest first, and no more than asked for,
- an entry is taken out only once,
- putting an entry in again keeps it until the later time,
- nothing is put in or taken out in a transaction that is dropped.

`purging.rs` holds the cases for the chore:

- every due entry of every kind is purged, batch after batch, and handed over with its kind,
- nothing is purged before its time,
- an entry whose purge was refused is left for the next run,
- a purge that leaves its entry behind stops the run.
