# chores

Background work that a feature hands over to the service. The service runs it from time to time, separate from any request.

A **`Chore`** has a name. Its `every()` says how often it runs. Its `carry_out()` does the work once and returns how many things it cleared. A feature hands over its chores through `wiring`, the same way it hands over its listeners.

**`Chores::started(chores)`** gives each chore its own loop. Each chore runs at once, and then again each time its `every` interval has passed. So an hourly chore never waits for a daily one. The number of things a chore cleared is logged. If a chore fails, a warning is logged and the chore runs again next time. A failing chore does not keep any other chore from running. **`stop()`** ends every loop at once. It does not wait for a slow chore's next turn.

This is how the boards delete their old tombstones:

```rust
use std::sync::Arc;

use chores::{Chores, ForgetOldTombstones};

let forget = ForgetOldTombstones::new(
    "forget-old-board-tombstones",
    chores::tombstones_by(move |before, at_most| {
        let boards = boards.clone();
        async move { boards.forget_tombstones_laid_before(before, at_most).await }
    }),
    clock.clone(),
);

let chores = Chores::started(vec![Arc::new(forget)]);
chores.stop().await;
```

A chore is for upkeep that nobody asked for at a specific moment. Deleting data that has long been settled is an example. Work that must happen before the server stops belongs in `wiring::Settling` instead.

**Tombstones.** Some features keep a tombstone of what was discarded, so a late message cannot bring it back. Such a feature hands over a **`ForgetOldTombstones`** chore, named after the feature. It comes with the **`Tombstones`** to delete, through `forget_laid_before(at)`. **`chores::tombstones_by(…)`** builds them from the core's own method, so the core needs no dependency on chores.

The chore runs hourly. It deletes the tombstones that were laid more than `TOMBSTONES_KEPT_FOR` ago. That is one day longer than `messaging::RETRIABLE_FOR`. The reasons:

- A refused change older than `messaging::RETRIABLE_FOR` is not retried.
- The extra day covers a change that happened a little after the tombstone was laid. An example is a scene that links an idea which was just discarded.
- So no message about a deleted tombstone can still arrive.

A tombstone's time is the moment the thing it marks went away. That is the `occurred_at` of the message that reported it.

The chore deletes in batches. `forget_laid_before(at, at_most)` deletes at most `at_most` tombstones of each kind, oldest first, in one short transaction. The chore asks for `TOMBSTONES_EACH_BATCH` at a time, until a batch comes back with fewer than that. So a large backlog never blocks a feature's store for long, but it is still cleared in one run. An example of such a backlog is a whole project deleted a month ago.
