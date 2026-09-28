# clock

Time behind a port, so that nothing reaches for the system clock on its own.

`Clock::now()` is the only way anything in Weaveling learns what time it is. `SystemClock` answers with the real one; `FixedClock` answers with whatever a test set, and can be moved forward mid-test. That is what lets a test assert an exact `created_at`, watch `updated_at` move when something is renamed, or age an outbox entry past its retention window without waiting ninety days.

It matters most for event sourcing: an event's `occurred_at` is written once and replayed forever, so the moment has to come from somewhere a test can pin.
