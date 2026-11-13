# transacting

Writes of one feature that belong together: either all of them are stored, or none is.

A **`Transaction`** is a unit of work. A feature's **`Transactions`** begins it. As many of that feature's ports as a use case needs write through it. **`finish()`** commits it. The core sees nothing of what is behind it: no connection and no database. A driven port's write method takes `&mut Transaction`, and its adapter reaches its backend through it. [ARCHITECTURE](../../ARCHITECTURE.md#a-transaction-in-the-port-signature--decided) explains why a transaction is part of the port signature, and the rules that come with it.

Accepting an invitation keeps the ticket and the account's events together like this:

```rust
let mut transaction = self.transactions.join(within).await?;
self.tickets.take(&mut transaction, digest).await?;
self.events.append(&mut transaction, id, expected, &stream).await?;
transaction.finish().await?;
```

**Begin or join.** `begin()` starts a transaction of its own. Some core methods can receive a caller's transaction. Such a method takes `within: Option<&mut Transaction>` and calls `join(within)`:

- Given `None`, it begins its own transaction, **`Joined::Own`**. Its `finish()` commits.
- Given the caller's transaction, it borrows it, **`Joined::Borrowed`**. Its `finish()` leaves the commit to the owner.

`Joined` derefs to the `Transaction`, so it is passed to the ports the same way.

**Finished, or rolled back.** Only `finish()` commits a transaction. If it is dropped unfinished, for example through an early `?`, everything it wrote is rolled back. It is never half committed. It is `#[must_use]`, so a transaction that is begun and never used causes a compiler warning. A transaction dropped unfinished is not logged. That is how every use case that fails on a `?` rolls back. A forgotten `finish()` shows up as writes that never get stored, and the feature's tests catch that.

**What follows a commit.** `after_commit(followup)` registers a future that runs once the transaction has committed. It never runs if the transaction is dropped. Examples: waking the outbox's relay, which must not go looking before there is anything to find, or taking a snapshot. `finish()` awaits each followup after the commit, in the order they were registered. A followup returns `()` and handles its own failure, because what follows a commit cannot undo it. A followup registered through a borrowed transaction waits for its owner's `finish()`.

**Retrying a lost race.** The owner of a transaction retries it, as [ARCHITECTURE](../../ARCHITECTURE.md) describes, through one macro:

```rust
transacting::retrying!(&*self.transactions, |transaction| {
    self.service.execute(Some(transaction), &id, command, agent).await?;
    Ok(())
})
```

The block runs in a fresh transaction. If it returns `Ok`, the transaction is finished. If it returns `Err`, the transaction is dropped and so rolled back. `?` and `return` end the try, not the method.

An error that is a **`LostRace`** (`lost_race()` is true) is retried, up to `retrying::TRIES` (five) times. Before each retry there is a pause. The pause is random, up to 10 ms at first, and the limit doubles after each try. So writers that collided spread out. Any other error is returned as the result. So is the fifth lost race. The error type takes a `Failure` through `From`.

*Lost races on PostgreSQL too, since 2026-11-04.* A try is also retried in two more cases:

- when its transaction itself lost the race, whatever error the work returned,
- when its commit lost the race, `Failure::LostRace`.

Adapters report a failed query with their own errors, mostly as text. So the race is not detected from those errors, but from `Transaction::lost_race()`. Such a race is retried up to five times, even where fewer tries are set. The reason: a write aimed at a version only learns on the next try whether that version has moved on.

**`joining!(transactions, within, |transaction| { … })`** runs the block in the transaction it was given, once. A lost race is then left to the owner. If it is given `None`, it runs the block through `retrying!`, in a transaction of its own.

`retrying!(transactions, tries, |transaction| { … })` sets the number of tries. One try means no retry at all. A write aimed at a version is tried once, as `eventsourcing::tries_for(expected)` says, and its lost race is returned as the result.

It is a macro, not a function that takes an async closure. Such a closure, borrowing `&self`, creates a future that is not `Send` on stable Rust. When the loop is written out where it is used, it can borrow whatever it likes.

**Backends.** There is one `Transactions` per feature, on that feature's own database.

- **`PostgresTransactions`** (feature `postgres`) begins on the pool.
  - *It is serializable since 2026-11-04.* Every connection in a feature's pool runs `postgres::SERIALIZABLE` when it connects. `wiring` and the test harness do this. So two transactions that each read what the other writes cannot both commit. The one that loses is retried by `retrying!`. This makes a plain check before a write as safe as on SQLite and in memory.
  - A lock taken in the transaction no longer lets a second writer wait and then read what the first one committed. The second writer's snapshot was already taken by then. So where many writers write one row at once, such as one scene, they lose and retry instead of waiting.
  - A statement run on the pool outside any `Transaction` is a transaction of its own. The deliveries, the outbox relay and the throttle run statements like that. Such a statement goes through `postgres::retried(|| …)`. It is retried after a lost race, the same way `retrying!` retries a transaction.
  - **`Tried<E>`** lets work whose error knows nothing about races run under `retrying!`, such as an adapter's work in a test. The work returns `Tried::Refused`. A failed transaction is `Tried::Failed`. `answered` turns the two into one error.
  - *Before that date*, PostgreSQL used its default isolation, READ COMMITTED. Under it, two writes that each read before they write could both read before either one commits. So each of them took `postgres::one_at_a_time(&mut transaction, key)`, an advisory lock. *That lock was removed the same day, together with its last caller.*
  - An adapter reaches its connection through `postgres::connection(&mut transaction)`. It gets the connection, not the transaction, because only the core ends the transaction.
  - *Since 2026-11-04 the connection is a `postgres::Connection`.* It is an `Executor`, like the sqlx connection it wraps. It marks the transaction as lost when a statement fails with a serialization failure (`40001`) or a deadlock (`40P01`). The commit then returns `Failure::LostRace`.
  - A read that decides a write locks what it reads, with `FOR UPDATE`, or is guarded by a constraint.
- **`SqliteTransactions`** (feature `sqlite`) begins with `BEGIN IMMEDIATE`. It takes the write lock right at the start. So a second writer waits, and then reads what the first one left.
  - *It also waits for its turn in the process first, without holding a connection (2026-11-02).* Every `SqliteTransactions` on one file queues on one lock before it takes a connection from the pool. Before this, writers that waited for SQLite's single writer held the pool's connections. Meanwhile the active writer needed one more connection to read. Under load this starved it until the busy timeout, and every waiting writer failed.
  - An adapter reaches its connection through `sqlite::connection(&mut transaction)`.
- **`InMemoryTransactions`** holds the feature's single lock for as long as a transaction lasts. So writers are serialised, as they are on SQLite.
  - Each write registers how to undo itself, with `memory::undo_on_rollback(&mut transaction, undo)`. A transaction dropped unfinished runs its undos in reverse order.
  - `InMemoryTransactions::refusing_commits()` refuses every commit. It rolls back and runs no followups. It is for testing what a use case does when its commit fails.
  - A reader outside the transaction may see a write that is not committed yet. Memory accepts this, because it only serves tests and development.

A transaction passed to an adapter of another backend is refused with **`Failure::BegunElsewhere`**. A backend that fails returns **`Failure::Backend`**.

**Conformance.** `suite.rs` holds the cases that every backend passes:

- what a finished transaction wrote is kept,
- what a dropped transaction wrote is gone,
- a borrowed transaction is finished only by its owner,
- what follows a commit runs only after a commit, each followup awaited in turn.

`retrying_tests.rs` holds the cases for retrying:

- a try that wins returns at once,
- a try that loses is retried in a fresh transaction, with what it wrote rolled back,
- the fifth loss is returned,
- another error is never retried,
- a retry works when spawned on another task,
- the pauses stay within their bounds and are spread out.

Each backend adds cases of its own:

- memory and SQLite: a second writer waits for the first,
- memory: undos run in reverse order,
- PostgreSQL: a statement that was executed or fetched, and a commit that lost a race, are retried, whatever the work returned. Another failure is not retried.
