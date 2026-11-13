# email

An email address, checked once and then trusted.

`EmailAddress::new` trims what was typed and lowercases it. It refuses anything longer than 254 characters. It requires a shape like `name@example.com`:

- something before the `@`,
- a domain with a dot, where the dot is neither at the start nor at the end,
- no second `@`,
- no space or control character anywhere.

A `+` is fine, because many people use addresses like `ada+weaveling@example.com` to sort their mail. The errors are `Malformed` and `TooLong { max, actual }`. The features translate them into their own errors for the client.

The typed text goes in. An address or an error comes out:

```rust
use email::{EmailAddress, InvalidEmailAddress};

let ada = EmailAddress::new("  Ada@Example.com ")?;
assert_eq!(ada.as_str(), "ada@example.com");

assert_eq!(EmailAddress::new("ada@example"), Err(InvalidEmailAddress::Malformed));
```

**It is lowercased on the way in, so one address is one key.** To every mail sender, `Ada@Example.com` and `ada@example.com` are the same mailbox. So they must also be the same account, the same throttle and the same invitation. Because this happens here, no store and no comparison has to remember to do it.

**It does not check that the address exists.** That is the job of the code that is mailed when someone signs up, or of the link in an invitation. This type only refuses addresses that could never be delivered.

**It is a library, not part of one feature,** since [3d-2](../../ROADMAP.md#milestone-17--accounts-members-and-roles). Accounts sign up with an address, and invitations are sent to an address. Both must follow the same rules. But one feature's core must not depend on another feature's core. `accounts_core` re-exports it, so code there reads the same as before.
