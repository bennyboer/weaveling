# invitations

A way in for someone who was asked, on a server where not everyone may sign up.

An operator invites an address to the instance; the address gets a link, and whoever follows it may sign up with that address and no other. Later an owner will invite into a project for a role, the same way.

**`Invitation` is an aggregate:** `Issued` to an address, then `Accepted` once, or `Withdrawn`. It stands open seven days from being issued; *expired* is not something that happens to it but something read from the time, so nothing has to run at midnight for a link to stop working. One taken up is not withdrawn, since the account it made stays and is locked or deleted as any other.

**The link carries a token, and only its digest is kept,** in `InvitationLinks` beside the events, as a session's is: 256 random bits in the link, their SHA-256 in the store. Whoever reads the store cannot follow a link from it.

**Following the link confirms the address,** since the link reached it, which is all the code mailed at signing up would show.

**Invitations into a project must not tell an owner whether an address already has an account,** answering alike either way; an operator, who sees every account anyway, is told.

Being built in [3d](../../ROADMAP.md#milestone-17--accounts-members-and-roles): the domain and the links' store stand; the feature's wiring, REST, the client and cleaning up expired invitations follow.

**Crates:** `core` (aggregate, token, links port) · `adapters/links` (the links' store in memory, on PostgreSQL and on SQLite).
