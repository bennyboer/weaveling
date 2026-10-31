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

**Backends.** One `Transactions` per feature, on that feature's own database:

- **`PostgresTransactions`** (feature `postgres`) begins on the pool at PostgreSQL's default isolation; an adapter reaches its connection through `postgres::connection(&mut transaction)`, the connection rather than the transaction, since only core ends it. A read that decides a write locks what it reads, `FOR UPDATE`, or is guarded by a constraint.
- **`SqliteTransactions`** (feature `sqlite`) begins with `BEGIN IMMEDIATE`, taking the write lock as it begins, so a second writer waits and reads what the first left; an adapter reaches its connection through `sqlite::connection(&mut transaction)`.
- **`InMemoryTransactions`** holds the feature's one lock for as long as a transaction lasts, so writers are serialised as SQLite's are; each write registers how to undo it with `memory::undo_on_rollback(&mut transaction, undo)`, and a transaction dropped unfinished runs its undos last first. A reader outside the transaction may see a write not yet committed, which memory, serving tests and development, accepts.

A transaction handed to an adapter of another backend is refused with **`Failure::BegunElsewhere`**; a backend that fails is **`Failure::Backend`**.

**Conformance.** `suite.rs` holds the cases every backend passes: what a finished transaction wrote is kept, what a dropped one wrote is gone, a borrowed transaction is finished only by its owner. Each backend adds what is its own: memory and SQLite that a second writer waits for the first, memory that undos run last first.
