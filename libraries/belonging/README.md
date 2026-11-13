# belonging

What another feature's things belong to, based on the messages this feature has received.

Boards, the outline and scenes link to ideas and scenes that they do not own. They must not link one that belongs to another project (M5 of the [security review](../../ROADMAP.md)). They cannot ask the feature that owns the thing without a call across the seam. So each of them keeps its own record of what it has received instead.

`remember(transaction, kind, id, owner_kind, owner)` stores what a thing belongs to, inside the caller's [transaction](../transacting/README.md). For example, it stores that an `idea` belongs to a `project`. The last message received wins. A thing has at most one owner of each kind. So it may belong to a project and also to something else of another kind.

`forget(transaction, kind, id, at)` deletes all owners of a thing, of every kind. It is called once the idea is discarded or the scene is deleted. So nothing is left of a deleted project. It also lays a tombstone, so a capture message that arrives again late does not bring the thing back.

`owner_of(kind, id, owner_kind)` returns the owner. It returns nothing for a thing this feature has not received a message about yet.

**Unknown is not refused.** A board may pin an idea that was captured a moment ago, before the capture message has reached the board. So a feature refuses only things it knows to belong elsewhere.

**Each feature creates the table in its own database.** This works like the [registry](../registry/README.md). So this is a shared mechanism, not shared data. `kind` keeps ideas and scenes apart. The migration ledger is `_sqlx_migrations_belongings`.

**The tombstones are deleted together with the feature's own tombstones.** Each feature's tombstone chore calls `forget_tombstones_laid_before(transaction, before, at_most)`, next to the call for its catalog. This happens once no late message can arrive any more.
