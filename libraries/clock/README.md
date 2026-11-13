# clock

Time behind a port, so that nothing reaches for the system clock on its own.

`Clock::now()` is the only way anything in Weaveling learns what time it is. `SystemClock` answers with the real one; `FixedClock` answers with whatever a test set, and can be moved forward mid-test. That is what lets a test assert an exact `created_at`, watch `updated_at` move when something is renamed, or age an outbox entry past its retention window without waiting ninety days.

A test pins the moment and moves it on:

```rust
use std::sync::Arc;

use clock::{Clock, FixedClock};
use time::{Duration, OffsetDateTime};

let clock = Arc::new(FixedClock::new(OffsetDateTime::UNIX_EPOCH));
let for_the_ideas: Arc<dyn Clock> = clock.clone();

clock.set(OffsetDateTime::UNIX_EPOCH + Duration::hours(1));

assert_eq!(clock::text::serialize(clock.now()), "1970-01-01T01:00:00Z");
assert_eq!(clock::text::written(clock.now()), "1970-01-01T01:00:00.000000000Z");
```

A test writes a moment as **`clock::at(seconds)`**, which is that many seconds after the Unix epoch. Tests use this instead of their own helpers.

It matters most for event sourcing. An event's `occurred_at` is written once and replayed forever. So the moment has to come from somewhere a test can pin.

**`text` is how an instant is stored in a database that has no time type of its own.** `text::written(at)` returns fixed-width UTC, such as `2026-10-07T11:22:33.123456789Z`. `text::read` parses it back. The fixed width matters: SQLite compares these values as strings. A claim's expiry and a retention cut-off rely on text order being the same as time order. It lives here because the event store, the outbox and the deliveries all write instants into SQLite. `clock` is the one time library they all already depend on.

**`text::serialize(at)` is how an instant is sent to others.** Every contract (a REST response, a published event, a refusal) carries instants as RFC 3339 strings, such as `2026-10-07T11:22:33Z`. `text::read` parses those back too. It is a separate function from `written` on purpose. Storage needs fixed width for its sort order. A contract needs the common form. Either may change without the other.
