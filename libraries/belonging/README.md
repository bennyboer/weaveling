# belonging

What another feature's things belong to, as far as this feature has heard. Boards, the outline and scenes link to ideas and scenes they do not own, and must not link one of another project (M5 of the [security review](../../ROADMAP.md)); they cannot ask the feature that owns it without a call across the seam, so each keeps what it hears instead.

`remember(transaction, kind, id, owner_kind, owner)` keeps what a thing belongs to, inside the caller's [transaction](../transacting/README.md): an `idea` to its `project`, the last heard winning. A thing has at most one owner of each kind, so it may belong to a project and to something else beside it. `forget(transaction, kind, id, at)` lets go of its owners of every kind once the idea is discarded or the scene deleted, so nothing stays of a deleted project, and lays a tombstone, so a capture heard again late does not bring it back; `owner_of(kind, id, owner_kind)` answers it, or nothing for a thing not heard of yet. **Not heard of is not refused:** a board pins an idea it captured a moment ago, before the capture has reached it, so a feature refuses only what it knows to belong elsewhere.

**Each feature lays the table down in its own database**, as with the [registry](../registry/README.md), so this is a shared mechanism rather than shared data; `kind` keeps an idea and a scene apart. The migration ledger is `_sqlx_migrations_belongings`.

**The tombstones are let go of with the feature's own:** `forget_tombstones_laid_before(transaction, before, at_most)` is called by each feature's tombstone chore beside its catalog's, once nothing heard late can come any more.
