# clock

Time behind a port, so that nothing reaches for the system clock on its own.

`Clock::now()` is the only way anything in Weaveling learns what time it is. `SystemClock` answers with the real one; `FixedClock` answers with whatever a test set, and can be moved forward mid-test. That is what lets a test assert an exact `created_at`, watch `updated_at` move when something is renamed, or age an outbox entry past its retention window without waiting ninety days.

It matters most for event sourcing: an event's `occurred_at` is written once and replayed forever, so the moment has to come from somewhere a test can pin.

**`text` is how an instant is written down where a database has no time type of its own.** `text::written(at)` gives fixed-width UTC — `2026-10-07T11:22:33.123456789Z` — and `text::read` takes it back. Fixed width matters: SQLite compares these as strings, so a claim's expiry and a retention cut-off rely on text order being time order. It lives here because the event store, the outbox and the deliveries all write instants into SQLite, and `clock` is the one time library they all already depend on.

**`text::serialize(at)` is how an instant travels.** Every contract — a REST response, a published event, a refusal — carries instants as RFC 3339 strings such as `2026-10-07T11:22:33Z`, and `text::read` takes those back too. It is a separate function from `written` on purpose: storage wants fixed width for its sort order, a contract wants the common form, and either may change without the other.
