# registry

One authoritative answer to "which id owns this key". It is for the cases where a projection cannot be trusted to give that answer.

`claim(transaction, kind, key, id)` writes the claim inside the caller's [transaction](../transacting/README.md), if the key is free. If the key is taken, it returns the id that already holds it. So **the loser of a race gets the winner's id instead of an error**, and can go and read that one. A claim never changes hands. `holder(kind, key)` reads a claim without making one.

The boards feature claims a project's board. If the claim is its own, it starts the board in the same transaction. So no claim ever points at a board that was never started:

```rust
use registry::Registry;

let mine = BoardId::generate(self.clock.now());
let mut transaction = self.transactions.begin().await?;
let held = self
    .registry
    .claim(&mut transaction, "board", project.as_str(), &mine.to_string())
    .await?;
let board: BoardId = held.parse()?;
if board == mine {
    self.events.begin(Some(&mut transaction), &AggregateId::from(&board), start, agent).await?;
}
transaction.finish().await?;

let found = self.registry.holder("board", project.as_str()).await?;
```

**It exists because find-or-start was broken.** `BoardService::open` and `OutlineService::open` used to ask the catalog whether the project already had a board. But the catalog is a projection, fed by a message. So a second open that arrived before the projection was updated started a *second* board. Inline publishing hid this bug, until [M11b](../../ROADMAP.md#milestone-11b--one-flow-in-every-mode) made both modes take the same path. A claim cannot lag behind the way a projection can.

**Each feature creates the table in its own database.** So this is a shared mechanism, not shared data. `kind` keeps a project's board and its outline apart within one feature's schema, so they do not hide each other. The migration ledger is `_sqlx_migrations_claims`. The reason: a feature runs the event store, its catalog and this registry in one schema. With a shared ledger, sqlx would see one version 1 changed into another.

There are three adapters with one conformance suite: in memory, PostgreSQL and SQLite. For each of the two durable ones, the suite runs eight concurrent claims on a single key, and asserts that all eight get the same id. On SQLite the claim is the same single upsert with `RETURNING`. Inside a transaction, a second claim on the key waits for the first one to commit or roll back. So it either gets a winner whose board exists, or it wins itself. A claim in a transaction that is dropped was never made.

**The project sweep uses it too, for the same reason.** Discarding a deleted project's board and outline used to start from the catalog. A board opened a moment before its project was deleted was not in the catalog yet. So it was never found, and it outlived its project. The claim is written when the board is opened. So `BoardService::board_of` and `OutlineService::outline_of` answer from the claim, and cannot lag behind.
