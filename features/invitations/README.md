# invitations

An invitation is a way in for someone who was asked, on a server where not everyone may sign up.

An operator invites an address to the instance.
The address receives a link.
Whoever follows the link may sign up with that address, and with no other address.
Later, an owner will invite people into a project for a role, in the same way.

**`Invitation` is an aggregate.**
It is first `Issued` to an address. After that, it ends in one of these ways:

- `Accepted`, once.
- `Withdrawn`.
- `Declined` for a reason. Either the address already has an account (`address-has-account`), or the person it was for turned it down (`turned-down`). The outcome keeps the reason, so the list can show why.

An invitation stays open for seven days after it is issued.
*Expired* is not an event that happens to it. It is read from the current time.
So nothing has to run at midnight for a link to stop working.
An invitation that was taken up is not withdrawn.
The account it created stays, and is locked or deleted like any other account.

**The link carries a token, and only its digest is stored.**
The digest is stored in the `Issued` event, just like a session's.
The link holds 256 bits; the store holds their SHA-256.
Someone who reads the store cannot follow a link from it.
*Decided on 2026-11-02: the token is derived, not drawn. The user chose this over a token drawn anew on resending, and over a token carried by the outbox.*
The token is the HMAC-SHA-256 of the invitation's id under the feature's one `TokenKey`.
So it can be derived again whenever the mail is sent.
A crash between issuing the invitation and sending its mail loses no link.
The key is drawn once, the first time it is needed.
It is stored in `token_key`, as one row, on PostgreSQL and on SQLite (`TokenKeys`, `PostgresTokenKeys`, `SqliteTokenKeys`).
In memory, each process draws its own key (`InMemoryTokenKeys`).
So a link mailed before a restart leads nowhere after it.
**A key drawn anew, or a lost row, makes every link not yet taken up lead nowhere.**
The old digests no longer match.
The people those links were for have to be invited again.
`invitation.issued` carries the address, the digest and the time until which the invitation is valid.
So accounts can keep a ticket for it without asking the invitations feature.

**Accepting is learned from a message, not requested.**
Accounts publishes `account.invitation.taken.up` whenever an account takes an invitation up.
That happens either when an address signs up through the ticket, or when an account that is already signed in takes it up.
The listener `accept-taken-up-invitation` receives only that message.
It records `Accepted` as of the time it was taken up, however late the message arrives.
`Accepted` carries the invitation's scope.
It is published as `invitation.accepted`, with the account as its agent.
So members can let the account join a project's members from the message alone.
If the message arrives again, or crosses a withdrawal on its way, nothing changes.
Declining is learned the same way, from `account.invitation.declined`, by the listener `decline-invitation`.
A declined invitation is closed. Withdrawing it changes nothing, and it is never accepted.
A project's invitations and the instance's invitations are to follow there too.

**Whoever holds the link may turn it down without an account.**
The route is `POST /invitations/decline` with the token.
It is served to anyone, because having the link proves that it reached them.
The list finds the invitation by the digest of the token.
The invitation is declined with `turned-down`, in the system's name, because there is no account to name.
Doing it a second time changes nothing.
A link that has run out is refused with `invitation-expired`.
A link nobody was given, or one already taken up or withdrawn, is refused with `invitation-invalid`.
This route is not throttled, because a token is 256 random bits.

**The operators' list is a read model** (`InvitationCatalog`).
It holds every invitation at its latest version, as an `InvitationSummary`: to whom, since when, until when, and its status.
It lists the open and the declined invitations, newest first, in pages.
It leaves out the accepted and the withdrawn ones, but does not forget them.
So a message that arrives late cannot bring one back.

**An invitation knows which account issued it.**
This is read from the agent of its issue event, `issued_by`. It is empty when the system issued the invitation.
The snapshot keeps it, so the list can show who invited.
The summary carries it as `issued_by`, stored in the list's `issued_by` column.

`open_to` returns the invitations to an address that are still open, newest first.
It uses an index by address.
So the account with that address can be told what it is invited to.

**`GET /invitations/mine`** returns exactly that for the signed-in account.
It uses the address the invitations feature knows for that account, learned from the sign-up messages.
It returns the account's invitations into projects that are still open and have not run out, newest first.
Each one has:

- the project's name, if known,
- the role,
- the name of who invited, if known,
- until when it is valid (`InvitationToYouDTO`).

An invitation to the instance is not an invitation into anything for an account, so it is not returned.
An account the invitations feature knows nothing about gets nothing.
Someone who is not an account gets nothing either.

**And live:** the topic `invitations` with `mine`, on the one socket `/api/live`, follows a channel for the signed-in account.
The channel is keyed by the account's own address (`InvitationsTopic`).
`TellingInvitations` wraps the list.
Each time the list stores an invitation to that address, `TellingInvitations` sends a frame on that topic, `of` the invitation's id.
So a frame only arrives once the list shows the change.
The client then asks for `mine` again.
The address is only a key inside the process. It is never sent.
An account the invitations feature does not know yet finds nothing to follow (`not-there`), until its sign-up message arrives.

**An invitation retires** once nobody needs it any more.
It records `Retired`, published as `invitation.retired`.
Each summary says when it retires, `retire_after`:

- An open invitation retires a month after it runs out. So it is listed as expired for that month.
- A declined invitation retires a month after it was declined.
- An invitation taken up or withdrawn retires a day later.
- But no invitation retires before it runs out. So an issue message that arrives late can never open a withdrawn link again.

The chore `retire-due-invitations` asks the list which invitations are due, and retires them.
The list forgets an invitation when it receives the message that it retired.
Accounts then drops its ticket.

**A retired invitation keeps no address.**
Retiring replaces the address with `deleted@invitation.invalid`, as a deleted account does.
`retire` compacts the invitation in the same transaction: it writes a snapshot and deletes every event before it.
So nothing that is left holds the address.
Its digest, `issued_by` and origin stay. None of them is personal.

**A retired invitation leaves a tombstone.**
`forget(invitation, at)` lays the tombstone and deletes the summary in one transaction.
`remember` refuses an invitation that has a tombstone.
So a summary that arrives late cannot bring a retired invitation back, where it would be due for retiring again every round.
On PostgreSQL, both transactions are serializable.
If a `remember` read no tombstone before a `forget` committed one, the `remember` loses the race.
It is tried again, and then it finds the tombstone.
(*The lock was removed on 2026-11-04, when every PostgreSQL transaction became serializable.* Before that, a lock on the invitation did this.)
On SQLite, only one writer runs at a time, which keeps them apart.
`forget_invitation_tombstones_laid_before` forgets the old tombstones, oldest first.
The chore `forget-old-invitation-tombstones` forgets them together with the projects' tombstones.
It does this a day after a change about them can no longer be retried.

The listener `catalogue-invitation` receives every change to an invitation.
It stores the invitation's current state, read from its events, not from what the message was about.
The invitation keeps the digest of its token, never the token itself, in its state and in its snapshot.
The list keeps the digest too, one per invitation.
So `with_digest` finds the invitation a link was for, whatever its status, until it is retired and forgotten.

`POST /invitations` answers 201 with the invitation as the list will show it.
So a page does not have to wait for the list to receive it.
`GET /invitations` serves the list to the operators, a hundred at a time.
Each entry has its status: `open`, `expired` or `declined` with the reason.
`expired` is read from the until time, and the invitation holds up to its last moment.

**Beside them it keeps who the accounts are.**
It stores a `Person` per account: its name and address, as accounts last published them (`remember_person`).
`people` returns the ones asked for that it knows, by account.
So an invitation can show who issued it and whose address it was sent to.
On PostgreSQL and SQLite this is stored in `people`.
The listener `invitations-remember-who-signed-up` receives `account.signed.up` and `account.signed.up.by.invitation`.
They carry the name and the address under `personal`.
The listener passes them to `InvitationsCore::remember_person`.
Accounts are never asked.
The listener's name says which feature it belongs to, because members receives the same sign-ups with a listener of its own, and a listener's name is also its queue's name.

**A deleted account is forgotten.**
`invitations-forget-deleted-account` receives `account.deleted`.
It has `InvitationsCore::forget_person` forget the account's name and address.

**And the projects' names**, because the person at an invitation's address may not be able to read its project yet.
`remember_project_name` stores a project's name by the project's version.
A newer name replaces an older one. A name that arrives late changes nothing.
A deleted project leaves a tombstone, `lay_project_tombstone`.
The tombstone forgets the project's name and stores `laid_at`, the moment the project was deleted.
So no name that arrives late brings it back.
`forget_project_tombstones_laid_before` forgets the old tombstones, oldest first.
It is used by the [`ForgetOldTombstones`](../../libraries/chores) chore, as in the other features.
`project_names` returns the names of the projects asked for that have one, by project.
On PostgreSQL and SQLite these are stored in `project_names` and `project_tombstones`.
The listener `remember-project-names` receives `project.started` and `project.renamed`, and stores the name by the project's version.
It also receives `project.deleted`, and lays the tombstone as of the time the project was deleted.
The chore `forget-old-invitation-tombstones` forgets the old ones.

**A deleted project's invitations are withdrawn.**
When `remember-project-names` lays a project's tombstone, it also asks, in the same transaction and through the outbox, for the project to be swept: `follow-up.invitation.sweep-project`.
The listener `withdraw-invitations-of-deleted-project` receives that follow-up.
So it reads the list only after the tombstone has committed.
`InvitationsCore::sweep` then withdraws, as the system, every invitation the project's list still holds, page after page.
An invitation taken up in the meantime is left as it is.
One already withdrawn or declined is not changed.
So accounts ends the tickets, and no link opens into a project that is gone.
No new invitation is issued into it afterwards either: `invite_to_project` treats a project with a tombstone as one that does not exist.

An invitation may be issued so shortly before the deletion that the list had not received it when the sweep ran.
That invitation is withdrawn once the list receives it.
When an invitation into a project is catalogued, the same transaction asks for `follow-up.invitation.withdraw-if-project-gone`.
The listener `withdraw-invitation-if-project-gone` receives it once the summary has committed.
If the project has a tombstone, it withdraws the invitation as the system (`withdraw_if_project_gone`).
An invitation already taken up is left alone.
So each side reads only after its own write has committed.
Of the two racing writes, the summary and the tombstone, one always sees the other.

The follow-ups' routing keys begin with `follow-up.`.
So `catalogue-invitation`, which receives `invitation.#`, never receives them.
They belong to this feature alone. They are declared in its messaging adapter, not in its contract.

**The link is `{origin}/invitation#<token>`.**
The token comes after `#`, and a browser never sends that part.
So the token reaches no access log and no `Referer`.
The page sends it only in the body of the sign-up request.

**Following the link confirms the address,**
because the link reached it.
That is all that the code mailed at signing up would prove.

**Invitations into a project must not tell an owner whether an address already has an account.**
They answer the same way in both cases.
An operator, who can see every account anyway, is told.

Being built in [3d](../../ROADMAP.md#milestone-17--accounts-members-and-roles). What stands so far:

- the domain, the codec, publishing, the service and REST,
- a server with accounts serves `POST /api/invitations` and `DELETE /api/invitations/{id}` to its operators,
- whoever follows a link signs up through it, and the invitation is then recorded as accepted,
- an invitation to an address that already has an account is recorded as declined,
- the operators can list the invitations,
- in the web client, operators invite, see and withdraw invitations on the instance's invitations page,
- whoever follows a link signs up and signs in on the page it opens.

Cleaning up expired invitations follows.

**Only an operator invites.**
This is read from the `Acting` that the accounts' gate passes on with the request. Accounts is never asked.
**An address can be invited three times before it has to wait.**
A throttle of the invitations' own, from [`throttling`](../../libraries/throttling), enforces this.
So if an operator's account is taken over, it cannot flood a stranger's inbox.
**The link is made from the `Origin`** of the operator's request.
An operator is trusted, so the server needs no setting with its own address.

**Messages say what happened to which invitation, never to whom it was sent.**
Messages are kept, and refused ones are shown.
So the address stays only in the events.

**Into the instance, or into a project.**
An invitation has a scope:

- the instance. Only an operator can issue such an invitation.
- a project, with the role it invites as: owner, author or reader. The project's owners issue it (`Manage`, asked of the `Authorizer`).

The invitations feature keeps its own record of who owns what, through [`access`](../../libraries/access/README.md).
`invitations-remember-roles` receives the members' messages, and `invitations-remember-trash` the projects' messages.
Where there are accounts, the `KeptAccess` over that record is the feature's `Authorizer`.
Where there are no accounts, everyone is allowed.
`forget-old-invitation-tombstones` also forgets what has ended there, beside the feature's own tombstones.

`POST /api/invitations` with a scope of kind `project`, naming the project and the role, invites into that project.
The scope is one tagged value wherever it travels: `{ "kind": "instance" }` or `{ "kind": "project", "project": …, "role": … }`.
`GET /api/invitations?project=` lists a project's invitations to its owners.
The operators' list shows only the instance's invitations.
Withdrawing checks permission according to the invitation's scope.
A reader or an author is told that only an owner may do this (403 `not-an-owner`).
Someone who may not read the project gets "not there". This includes an operator who is not in the project.
A role nobody knows is 422 `role-unknown`.
The issue event records whether an operator issued it.
This matters because only an operator's invitation into a project may bring in a new address when signing up is by invitation only.
The mail says which role the person is invited as.
Declining because the address has an account applies only to the instance.
For a project, having an account is the usual case, and an owner is never told whether there is one.
Taking a project's invitation up comes next.

**One way in: `InvitationsCore`.**
Issuing, withdrawing and listing check whether the agent is an operator. They check it there and nowhere else.
An invitation taken up or declined, cataloguing, and retiring what is due all go through the same struct.
Declining and retiring run as `Agent::System`.
`InvitationService` only carries out the commands, in the transaction that `InvitationsCore` hands it. It also throttles and sends mails.
Nothing outside the core can reach it.
Errors are one enum, `InvitationsError`.

**An invitation into a project in the trash is not told.**
*2026-11-02, slice 6d.*
`remember-project-names` also receives `project.trashed` and `project.restored`.
It stores whether the project is in the trash (`remember_trash`, `is_in_the_trash`, in `project_trash` on PostgreSQL and SQLite).
It stores this by the project's version and only ever moves forward.
It stores it apart from the name, so a rename that arrives late is not lost to a newer trashing.
A project's tombstone forgets its trash status together with its name.
A trashing that arrives late does not bring it back.
`GET /invitations/mine` leaves out an invitation into a project in the trash, because accounts refuses to take it up until the project is restored.
Once the project is restored, the invitation is shown again.
New invitations into a trashed project are already refused, because the `Authorizer` refuses `Manage` on a project in the trash.
An invitation that is still open is withdrawn by the sweep after the purge, as before.
The live `mine` topic is told when a project is trashed, restored or renamed.
`open_into` names each address that has an open invitation into the project, once.
`TellingInvitations` tells each of them after the commit, with a frame `of` the project.

**Core owns each write's transaction.**
`InvitationsCore` begins every write's transaction.
Issuing checks the throttle first, outside the transaction, because an admitted try counts whatever happens next.
It then appends in its own transaction.
The origin that the link should lead back to is stored in the event.
Issuing sends no mail itself.

**`mail-issued-invitation` sends the mail.**
It is a kept listener on `invitation.issued`, run through the inbox like the others.
It reads the invitation, derives its token and mails the link. As a result:

- An invitation that was not stored is mailed to nobody.
- An invitation received twice is mailed once.
- A mail that could not be sent is refused, and is sent when the issue message is offered again.
- An invitation withdrawn, declined, taken up or run out before its mail was processed is not mailed at all.

Withdrawing and retiring run through `transacting::retrying!`. They get five tries after a lost race. None of them names a version.
Forgetting both kinds of tombstone shares one transaction.

**Every listener but the sweep is run through its inbox.**
These listeners are `KeptHandler`s run through [`inbox::Inboxed`](../../libraries/inbox/README.md):
`mail-issued-invitation`, `accept-taken-up-invitation`, `decline-invitation`, `catalogue-invitation`, `withdraw-invitation-if-project-gone`, `invitations-remember-who-signed-up` and `remember-project-names`.
`inbox::Inboxed` begins the transaction and asks the feature's inbox whether the message was received before.
It then hands the transaction to the listener and finishes it.
If it loses a race, the delivery offers the message again.
Accepting, declining, cataloguing, withdrawing if the project is gone, and storing a person, a project's name or its tombstone all take the transaction as `within` and join it.
The follow-ups also go into the outbox in that transaction.
So recording the message commits together with its effects and its follow-ups, or none of them commits.
`withdraw-invitations-of-deleted-project` stays a plain listener.
Its sweep withdraws page after page, each page in a transaction of its own.
A withdrawal that was already done changes nothing.
The chore `forget-old-invitation-hearings` forgets the old records of received messages, through `HeardMessages`.
`TellingInvitations` tells its topic in an `after_commit`, so never for a write that was rolled back.

**Crates:** `core` (`InvitationsCore`, aggregate, token, service) · `contract` (routing keys, event DTO) · `adapters/store` (stored-event codec) · `adapters/catalog` (the list in memory, on PostgreSQL and on SQLite) · `adapters/messaging` (event publisher and listeners) · `adapters/rest` · `wiring` · `tests`.
