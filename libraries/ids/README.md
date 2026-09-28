# ids

Identifiers that are prefixed, short, and sorted by age.

The `id!` macro mints a newtype — `ids::id!(ProjectId, "project_")` — giving it parsing, `Display`, and a generator that takes the moment from a [`Clock`](../clock). Every id is a **UUIDv7**, so ids created later sort after ids created earlier, and **base62-encoded** to 22 characters rather than 36 of hyphenated hex.

Both choices are load-bearing. Time-ordering is what lets a catalog list "newest first" with `ORDER BY id DESC` and no timestamp column; a v7 id also carries its own creation moment, so `created_at` is recoverable from the id alone. The short encoding is why an id column costs 28 bytes rather than 45 — worth roughly 40MB per million outbox rows.

The one trap: base62 mixes cases, so an id column only sorts by age under `COLLATE "C"`. A glibc locale puts `a` before `A` and the ordering silently stops meaning anything.
