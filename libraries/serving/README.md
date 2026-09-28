# serving

The HTTP conventions every REST adapter shares, so that four features cannot drift apart on them.

- **`demanded(headers)`** reads `If-Match` into an `Option<Version>` — absent or `*` means "no expectation", a quoted number means "only if it still stands at this version", anything else is `Unreadable` and becomes a `400`.
- **`tag(version)`** writes the matching `ETag`.
- **`refusal(&ServiceError<E>)`** maps an event-sourcing refusal onto a status: not found is `404`, a domain refusal is `409`, a stale version is `412`, and anything else is logged and becomes a `500` that says nothing.

Small on purpose. Optimistic concurrency is only honest if every route spells it the same way, and a feature that hand-rolled its own `If-Match` parsing would be a feature whose conflicts behave differently from the rest.
