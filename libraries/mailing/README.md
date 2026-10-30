# mailing

Mail behind a port, since Weaveling has no mail provider of its own.

A feature hands a `Mail` (to whom, a subject, a body) to a `Mailer` and learns whether it went, `Unsent` if not. Which mailer it is the server decides at start, by `WEAVELING_MAIL_DROP`:

- **`LoggingMailer`**, unless told otherwise, writes each mail into the log. Enough to try a server alone: the code to confirm an address is right there beside the request that asked for it.
- **`DroppingMailer`** leaves each mail as a JSON file in a folder, named so they sort in the order they were sent. The browser tests with accounts read the codes from there, and a self-hosted server can hand the folder to whatever sends mail on.
- **`FakeMailbox`** keeps them in memory for the Rust tests, which ask it what was `sent_to` an address.

The accounts send a mail, and a test asks the mailbox what went:

```rust
use std::sync::Arc;

use mailing::{FakeMailbox, Mail, Mailer};

let mailbox = Arc::new(FakeMailbox::default());
let mailer: Arc<dyn Mailer> = mailbox.clone();

mailer
    .send(Mail {
        to: "ada@example.com".to_owned(),
        subject: "Signing up to Weaveling".to_owned(),
        body: "Your code is 123456.".to_owned(),
    })
    .await?;

assert_eq!(mailbox.sent_to("ada@example.com").len(), 1);
```

**A provider comes later, as one more `Mailer`.** The port is the decision: nothing that sends a code, a reset or an invitation knows how mail leaves the machine, so choosing a provider touches only the server's wiring.
