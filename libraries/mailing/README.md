# mailing

Mail behind a port, because Weaveling has no mail provider of its own.

A feature hands a `Mail` (recipient, subject, body) to a `Mailer`. The mailer returns whether the mail was sent. If it was not, it returns `Unsent`. The server decides at startup which mailer to use, based on `WEAVELING_MAIL_DROP`:

- **`LoggingMailer`** is the default. It writes each mail into the log. That is enough to try out a server on your own: the code to confirm an address appears in the log right next to the request that asked for it.
- **`DroppingMailer`** stores each mail as a JSON file in a folder. The files are named so they sort in the order they were sent. The browser tests with accounts read the codes from there. A self-hosted server can hand the folder to some other program that sends the mail on.
- **`FakeMailbox`** keeps the mails in memory, for the Rust tests. A test asks it what was `sent_to` an address.

The accounts feature sends a mail, and a test asks the mailbox what was sent:

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

**A provider will come later, as one more `Mailer`.** The port is the actual decision. Nothing that sends a code, a password reset or an invitation knows how mail leaves the machine. So choosing a provider changes only the server's wiring.
