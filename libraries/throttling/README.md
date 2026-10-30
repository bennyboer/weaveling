# throttling

How often something may be tried before it has to wait, kept the same wherever something is guarded.

**The rule.** A `Rule` says how many tries are free. Past them each try must wait, a minute after the first one over, then two, four, eight, doubling up to an hour and no further, since a wait without end would lock out whoever owns an address, which anyone typing that address could do. The wait is counted from the last try and shrinks as time passes; tries a day old are forgotten. `Tries::wait_before_next(rule, now)` says how long, or nothing.

The invitations let three go free to an address, then make the next wait:

```rust
use throttling::{Rule, Throttles};

const INVITING: &str = "inviting";
const INVITING_RULE: Rule = Rule { free: 3 };

let now = self.clock.now();
if let Some(tries) = self.throttles.tries(INVITING, to.as_str()).await? {
    if let Some(wait) = tries.wait_before_next(INVITING_RULE, now) {
        return Err(InvitationsError::Throttled { wait });
    }
}
self.throttles.count(INVITING, to.as_str(), now).await?;
```

**The count behind a port.** `Throttles` counts tries by **what is guarded** and **about whom**, signing in and an address, say, so one person's mistakes never slow another down and one kind of try never another. Tries counted at the same moment are each counted, or sending them together would slip the throttle. Kept in memory, on PostgreSQL and on SQLite, each feature laying the table down in its own database under a ledger of its own.

**Per address, never per IP,** as decided for accounts: many people share an address behind a NAT, and a throttle by IP would let one slow them all.

**Why a library.** Accounts throttle signing in and sending codes; invitations throttle inviting an address. Each keeps a count of its own, in its own database, and each decides what is free, but the rule, how long a wait grows and when tries are forgotten, must be one.
