# email

An email address, checked once and then trusted.

`EmailAddress::new` trims what was typed and lowercases it, refuses anything longer than 254 characters, and asks for a shape like `name@example.com`: something before the `@`, a domain with a dot that neither starts nor ends it, no second `@`, no space or control character anywhere. A `+` is fine, since `ada+weaveling@example.com` is how many people keep their mail apart. It refuses `Malformed` or `TooLong { max, actual }`, which the features name for the client.

What was typed goes in, an address or a refusal comes out:

```rust
use email::{EmailAddress, InvalidEmailAddress};

let ada = EmailAddress::new("  Ada@Example.com ")?;
assert_eq!(ada.as_str(), "ada@example.com");

assert_eq!(EmailAddress::new("ada@example"), Err(InvalidEmailAddress::Malformed));
```

**Lowercased on the way in, so one address is one key.** `Ada@Example.com` and `ada@example.com` are the same mailbox to everyone who sends mail, so they must be the same account, the same throttle and the same invitation. Doing it here means no store and no comparison has to remember to.

**It is not a check that the address exists.** That is what the code mailed at signing up is for, or the link of an invitation; this only refuses what could never be delivered.

**A library rather than part of one feature** since [3d-2](../../ROADMAP.md#milestone-17--accounts-members-and-roles): accounts sign up with an address and invitations are sent to one, and both must hold the same rules, without one feature's core depending on another's. `accounts_core` passes it on, so code there reads as before.
