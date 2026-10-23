# invitations

A way in for someone who was asked, on a server where not everyone may sign up.

An operator invites an address to the instance; the address gets a link, and whoever follows it may sign up with that address and no other. Later an owner will invite into a project for a role, the same way.

**`Invitation` is an aggregate:** `Issued` to an address, then `Accepted` once, or `Withdrawn`. It stands open seven days from being issued; *expired* is not something that happens to it but something read from the time, so nothing has to run at midnight for a link to stop working. One taken up is not withdrawn, since the account it made stays and is locked or deleted as any other.

**The link carries a token, and only its digest is kept,** in `InvitationLinks` beside the events, as a session's is: 256 random bits in the link, their SHA-256 in the store. Whoever reads the store cannot follow a link from it.

**Following the link confirms the address,** since the link reached it, which is all the code mailed at signing up would show.

**Invitations into a project must not tell an owner whether an address already has an account,** answering alike either way; an operator, who sees every account anyway, is told.

Being built in [3d](../../ROADMAP.md#milestone-17--accounts-members-and-roles): the domain, the links' store, the codec, publishing, the service and REST stand, and a server with accounts serves `POST /api/invitations` and `DELETE /api/invitations/{id}` to its operators; signing up through the link, the list of open invitations, the client and cleaning up expired invitations follow.

**Only an operator invites,** read from the `Acting` the accounts' gate hands on with the request, never by asking accounts. **An address is invited thrice before it must wait**, by a throttle of the invitations' own from [`throttling`](../../libraries/throttling), so an operator's account taken over cannot fill a stranger's inbox. **The link is made from the `Origin`** of the operator's request, an operator being trusted, so the server needs no setting naming its own address.

**Messages tell what happened to which invitation, never to whom it was sent:** they are kept, and refused ones shown, so the address stays in the events alone.

**Crates:** `core` (aggregate, token, links port) · `contract` (routing keys, event DTO) · `adapters/store` (stored-event codec) · `adapters/links` (the links' store in memory, on PostgreSQL and on SQLite) · `adapters/messaging` (event publisher) · `adapters/rest` · `wiring` · `tests`.
