# throttling

How often something may be tried before it has to wait. The rule is the same everywhere something is guarded.

**The rule.** A `Rule` says how many tries are free. After those, each try must wait:

- one minute after the first try over the limit,
- then two minutes, four, eight, and so on, doubling each time,
- up to one hour, and no further.

The wait never grows beyond an hour. An endless wait would lock out the owner of an address. And anyone who types that address could cause it.

The wait is counted from the last try, and it shrinks as time passes. Tries that are a day old are forgotten. `Tries::wait_before_next(rule, now)` returns how long to wait, or nothing.

**Old tries are also deleted for good.** `Throttles::forget_tried_before(before, at_most)` deletes the counts whose last try was before `before`, oldest first. The chore `ForgetOldTries` does this every hour, for counts that are a day old (`Tries::FORGOTTEN_AFTER`). A feature names this chore and wires it over its throttles. Such counts no longer count for anything, and they name an address or an account. The accounts feature wires `forget-old-account-tries`. The invitations feature wires `forget-old-invitation-tries`.

The invitations feature allows three free tries per address. After that, the next try must wait:

```rust
use throttling::{Admission, Rule, Throttles};

const INVITING: &str = "inviting";
const INVITING_RULE: Rule = Rule { free: 3 };

match self
    .throttles
    .admit(INVITING, to.as_str(), INVITING_RULE, self.clock.now())
    .await?
{
    Admission::Admitted => {}
    Admission::Wait(wait) => return Err(InvitationsError::Throttled { wait }),
}
```

**Admitted and counted in one step.** `admit(guarded, about, rule, at)` decides based on the rule. It counts the try only when it lets the try through. Both happen as one step. This has two effects:

- Tries sent together cannot all read a count below the limit before any of them is counted.
- A try that is refused because it must wait is not counted. So impatience alone never makes the wait longer. Neither does someone who keeps typing another person's address.

`Tries::admitted` holds the decision. It returns what an admitted try leaves behind, or how long a refused try must wait. So every backend decides the same way:

- Memory decides under its lock.
- PostgreSQL and SQLite read, and then write only if nothing changed since. The write is either an insert that does nothing if another try got there first, or an update that only matches the count and moment that were read. If another try was counted in between, they read again.

`tries` reads the count. `clear` resets it, for example after a correct password.

**The count sits behind a port.** `Throttles` counts tries by **what is guarded** and by **whom it is about**. For example, signing in, and an address. So one person's mistakes never slow down another person. And one kind of try never slows down another kind. Tries sent at the same moment are admitted no more often than the rule allows. The conformance suite proves this by sending twenty at once. There are implementations in memory, on PostgreSQL and on SQLite. Each feature creates the table in its own database, under a ledger of its own.

**Per address, never per IP.** This was decided for accounts. Many people share one IP address behind a NAT. A throttle by IP would let one person slow down all of them.

**Why a library.** The accounts feature throttles signing in and sending codes. The invitations feature throttles inviting an address. Each keeps its own count, in its own database. Each decides what is free. But the rule must be one and the same: how long a wait grows, and when tries are forgotten.
