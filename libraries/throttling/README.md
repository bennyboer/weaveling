# throttling

How often something may be tried before it has to wait, kept the same wherever something is guarded.

**The rule.** A `Rule` says how many tries are free. Past them each try must wait, a minute after the first one over, then two, four, eight, doubling up to an hour and no further, since a wait without end would lock out whoever owns an address, which anyone typing that address could do. The wait is counted from the last try and shrinks as time passes; tries a day old are forgotten. `Tries::wait_before_next(rule, now)` says how long, or nothing. **And forgotten for good:** `Throttles::forget_tried_before(before, at_most)` deletes the counts last tried before then, oldest first, and the chore `ForgetOldTries`, which a feature names and wires over its throttles, does it every hour for those a day old (`Tries::FORGOTTEN_AFTER`), as they count for nothing any more and name an address or an account: accounts wire `forget-old-account-tries`, invitations `forget-old-invitation-tries`.

The invitations let three go free to an address, then make the next wait:

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

**Admitted and counted in one step.** `admit(guarded, about, rule, at)` decides by the rule and counts the try only when it lets it through, as one step: tries sent together cannot all read a count under the limit before any of them is counted, and a try refused for its wait is not counted, so impatience alone, or whoever keeps typing someone's address, never lengthens the wait. `Tries::admitted` holds the decision, what an admitted try leaves behind or how long a refused one waits, so every backend decides alike: memory under its lock, PostgreSQL and SQLite by reading and then writing only if nothing changed since, an insert that does nothing where another got there first and an update that matches the count and moment it read, reading again when another try was counted in between. `tries` reads the count and `clear` starts it over, after a right password, say.

**The count behind a port.** `Throttles` counts tries by **what is guarded** and **about whom**, signing in and an address, say, so one person's mistakes never slow another down and one kind of try never another. Tries sent at the same moment are admitted no more often than the rule lets, which the conformance suite proves by sending twenty at once. Kept in memory, on PostgreSQL and on SQLite, each feature laying the table down in its own database under a ledger of its own.

**Per address, never per IP,** as decided for accounts: many people share an address behind a NAT, and a throttle by IP would let one slow them all.

**Why a library.** Accounts throttle signing in and sending codes; invitations throttle inviting an address. Each keeps a count of its own, in its own database, and each decides what is free, but the rule, how long a wait grows and when tries are forgotten, must be one.
