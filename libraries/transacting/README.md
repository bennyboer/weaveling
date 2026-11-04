# transacting

Writes of one feature that hold together: either all of them land or none does.

A **`Transaction`** is a unit of work begun by a feature's **`Transactions`**, written through by as many of that feature's ports as a use case needs, and committed by **`finish()`**. Core sees nothing of what is behind it, no connection and no database: a driven port's write method takes `&mut Transaction`, and its adapter reaches its backend through it. Why a transaction sits in the port signature, and the rules that come with it, are in [ARCHITECTURE](../../ARCHITECTURE.md#a-transaction-in-the-port-signature--decided).

Taking up an invitation keeps the ticket and the account's events together like this:

```rust
let mut transaction = self.transactions.join(within).await?;
self.tickets.take(&mut transaction, digest).await?;
self.events.append(&mut transaction, id, expected, &stream).await?;
transaction.finish().await?;
```

**Begin or join.** `begin()` starts a transaction of its own. A core method a caller may hand its transaction to takes `within: Option<&mut Transaction>` and asks `join(within)`: given `None` it begins one, **`Joined::Own`**, which its `finish()` commits; given the caller's, it borrows it, **`Joined::Borrowed`**, which its `finish()` leaves for the owner to commit. `Joined` derefs to the `Transaction`, so it is handed to the ports the same way.

**Finished, or rolled back.** A transaction is committed only by `finish()`; dropped unfinished, through an early `?` say, what it wrote is rolled back, never half committed. It is `#[must_use]`, so one begun and never used is a compiler warning. One dropped unfinished is not logged: that is how every use case that fails on an `?` rolls back, and a `finish()` forgotten shows as writes that never land, which the feature's tests see.

**What follows a commit.** `after_commit(followup)` asks for a future to run once the transaction has committed, never if it is dropped: waking the outbox's relay, say, which must not go looking before there is anything to find, or taking a snapshot. `finish()` awaits each followup in the order they were asked for, after the commit, so a followup answers `()` and deals with its own failure: what follows a commit cannot undo it. One asked through a borrowed transaction waits for its owner's `finish()`.

**Retrying a lost race.** Whoever owns a transaction retries it, as [ARCHITECTURE](../../ARCHITECTURE.md) has it, through one macro:

```rust
transacting::retrying!(&*self.transactions, |transaction| {
    self.service.execute(Some(transaction), &id, command, agent).await?;
    Ok(())
})
```

The block runs in a fresh transaction, finished when it answers `Ok`, dropped and so rolled back when it answers `Err`; `?` and `return` end the try, not the method. An error that is a **`LostRace`** (`lost_race()` true) is tried again, up to `retrying::TRIES`, five, after a pause drawn at random up to 10 ms and twice as long after each try, so writers that collided spread out; any other error, and the fifth lost race, is the answer. The error type takes a `Failure` through `From`. *Lost on PostgreSQL too, 2026-11-04:* a try is also tried again when its transaction lost the race itself, whatever error the work answered, and when its commit did, `Failure::LostRace`; adapters answer a failed query in errors of their own, mostly as text, so the race is not read from them but from `Transaction::lost_race()`. `retrying!(transactions, tries, |transaction| { … })` sets how many tries, one being no retry at all: a write aimed at a version is tried once, `eventsourcing::tries_for(expected)` saying so, and its lost race is the answer. A macro rather than a function taking an async closure, since such a closure borrowing `&self` makes a future that is not `Send` on stable Rust; written out where it is used, the loop borrows what it likes.

**Backends.** One `Transactions` per feature, on that feature's own database:

- **`PostgresTransactions`** (feature `postgres`) begins on the pool at PostgreSQL's default isolation; `postgres::one_at_a_time(&mut transaction, key)` lets one transaction at a time past it for the key, until it is over, for two writes that each read before they write, as a catalog's upsert checking a tombstone and the forgetting that lays one, which under READ COMMITTED could both read before either commits (*one named helper since 2026-11-02, the user finding the raw `pg_advisory_xact_lock` in every catalog hard to read, over a follow-up message on every write*); an adapter reaches its connection through `postgres::connection(&mut transaction)`, the connection rather than the transaction, since only core ends it; *the connection is `postgres::Watched` since 2026-11-04,* an `Executor` like the connection it wraps, which marks the transaction lost when a statement fails as a serialization failure (`40001`) or a deadlock (`40P01`), as `one_at_a_time` and the commit answer `Failure::LostRace` then. A read that decides a write locks what it reads, `FOR UPDATE`, or is guarded by a constraint.
- **`SqliteTransactions`** (feature `sqlite`) begins with `BEGIN IMMEDIATE`, taking the write lock as it begins, so a second writer waits and reads what the first left; *and it waits its turn first in the process, without a connection (2026-11-02):* every `SqliteTransactions` on one file queues on one lock before it takes a connection from the pool, so writers waiting for SQLite's one writer do not hold the pool's connections while the one writing needs one more to read, which under load starved it until the busy timeout and failed every writer in line; an adapter reaches its connection through `sqlite::connection(&mut transaction)`.
- **`InMemoryTransactions`** holds the feature's one lock for as long as a transaction lasts, so writers are serialised as SQLite's are; each write registers how to undo it with `memory::undo_on_rollback(&mut transaction, undo)`, and a transaction dropped unfinished runs its undos last first. `InMemoryTransactions::refusing_commits()` refuses every commit, rolling back and running nothing that follows, for a test of what a use case does when its commit fails. A reader outside the transaction may see a write not yet committed, which memory, serving tests and development, accepts.

A transaction handed to an adapter of another backend is refused with **`Failure::BegunElsewhere`**; a backend that fails is **`Failure::Backend`**.

**Conformance.** `suite.rs` holds the cases every backend passes: what a finished transaction wrote is kept, what a dropped one wrote is gone, a borrowed transaction is finished only by its owner, and what follows a commit runs only after one, each awaited in turn. `retrying_tests.rs` holds the retry's: a won try answered at once, a lost one tried again in a fresh transaction with what it wrote rolled back, the fifth loss answered, another error never retried, a retry spawned on another task, and the pauses' bounds and spread. Each backend adds what is its own: memory and SQLite that a second writer waits for the first, memory that undos run last first, PostgreSQL that a statement executed or fetched and a commit that lost a race are tried again however the work answered it, and another failure is not.
