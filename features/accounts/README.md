# accounts

This feature is about who someone is on a Weaveling server.
It covers signing up, signing in, and the operator who looks after the instance.

Only a server with `WEAVELING_ACCOUNTS=on` has accounts.
Local mode has no accounts.
There, everything is done by the one person at the keyboard, `Agent::Local`.
Nothing in this feature runs in local mode.

**`Account` is an aggregate.** Like every feature, it is event-sourced. Its events are:

- `SignedUp`
- `SignedUpByInvitation`. The address is confirmed from the start, because the invitation link reached it.
- `EmailVerified`
- `PasswordReset`
- `BecameOperator`
- `StoppedBeingOperator`
- `Locked`
- `Unlocked`

The events say *that* a password was set or reset. They never contain the password.
**A sign-up is published with the name and the address under `personal`.**
Members need them to show who is in their list.
Nothing else personal is ever published.
**The secrets are kept beside the events.**
Only the latest value of each secret is stored, so an old log holds no old password hash.

These are stored beside the events, each behind its own port in `SecretStores`:

- the address an account holds,
- the Argon2id hash of its password,
- the pending codes,
- the sessions, stored by the digest of their token,
- the tries counted by [`throttling`](../../libraries/throttling),
- which accounts are appointed operators,
- the tickets for invitations. A ticket is stored by the digest of the link's token. It holds the invited address, the time until which it is valid, the project it invites into (if any), and whether an operator issued it.

A ticket is found by the digest of its link.
An account that takes an invitation up inside the application finds the ticket by its invitation instead (`digest_of_ticket`), as long as the ticket is still open.
A ticket can be taken only once.
It ends when it is taken or when its invitation is withdrawn.
An ended ticket stays ended.
Messages may be delivered more than once, so the same issue message may arrive again; it does not reopen a link that was closed.
When the invitation retires (`invitation.retired`), the ticket is deleted completely, whether it is open, ended or just a mark.
Invitations never retire before they run out.
So no message that arrives later can create a ticket that still opens.

The tickets are made from the invitations' messages, by the listener `keep-invitation-tickets`.
So accounts never have to ask the invitations feature anything.
The listener forgets a ticket when its invitation is withdrawn or declined.
The declined case covers an invitation that was turned down through its link by someone without an account.
Accounts would not hear about that case otherwise.

If an invitation goes to an address that already has an account, that account declines it.
It records `InvitationDeclined` with a `DeclineReason`.
The reason here is `AddressHasAccount`; the other reason, `TurnedDown`, is described below.
This is published as `account.invitation.declined`, with the invitation and the reason `address-has-account`.
No ticket is kept for it.
This only applies to an invitation to the instance.
An invitation into a project keeps its ticket no matter whose address it was sent to, because an account takes it up while signed in.

Signing up through an invitation into a project works the same way as through an invitation to the instance, and it takes the invitation up in the same way.
When signing up is by invitation only, only an operator's invitation can sign someone up.
An invitation from a project's owner is then refused with `InvitationNotByAnOperator` (403 `invitation-not-by-an-operator`).
That invitation stays open for its address.
If the address already has an account, the person is asked to sign in before any ticket is used.
That way, the link still works once they are signed in.

**An account takes an invitation up** with its link, while signed in (`take_up_invitation`). There are three conditions:

- It must be an invitation into a project. An invitation to the instance is only for signing up.
- It must have been sent to the account's own address. Otherwise it is refused with `InvitationForAnotherAddress`, which names the address it was sent to. The invitation stays open for that address.
- It must not have run out yet.

The ticket is used up.
The account records `InvitationTakenUp`, published as `account.invitation.taken.up`.
The account itself is the agent.
Signing up through an invitation also records `InvitationTakenUp`, right after `SignedUpByInvitation`.
So every invitation an account took up shows the same way in its history.
`SignedUpByInvitation` itself does not name the invitation.
The invitation is published only once, as taken up.

Routes for taking an invitation up:

- `POST /accounts/current/invitations/accept` with the link's token takes an invitation up for the signed-in account. It answers 204. An invitation sent to another address is refused with 403 `invitation-for-another-address`. The problem names that address in `to`, so the page can offer to sign in as it.
- `POST /accounts/current/invitations/{invitation}/accept` takes one up by its id. It is for an account that learned about the invitation inside the application. The same checks apply. But an invitation sent to another address is refused as `invitation-invalid`, as if it did not exist. An id can be guessed, but a link cannot. So this route does not reveal whom the invitation was sent to.

**Turning one down** goes through the same checks, with one difference: any invitation may be turned down, including one to the instance. Only an invitation into a project can be taken up.
Turning down uses the ticket up in the same way.
The routes are `POST /accounts/current/invitations/decline` with the link's token, or `POST /accounts/current/invitations/{invitation}/decline` by its id. Both answer 204.
The account records `InvitationDeclined` with the reason `turned-down`, and the account itself is the agent.
This is published as `account.invitation.declined`.
The invitations feature then marks the invitation declined, just as it does for one declined because the address already has an account.

**An address is claimed in the transaction that begins its account.**
So two sign-ups can never share an address.
The claim, the account's first events and its password are all stored together, or none of them is.
Signing up by invitation also takes the ticket in that same transaction.
A claim stores when it was made.
If a claim's holder never became an account, the next sign-up takes the claim over once five minutes have passed.
Only claims left over from before the claim and the account were one transaction still need this.

**A password reset is one transaction too.**
Its event and the new password are stored together.
The event ends the sessions begun before it and confirms the address.
If the new password cannot be stored, nothing changes: the old password still works, and the same code can finish the reset.
Before, the sessions were ended first.
But ending the sessions protected nothing while the old password still let its holder back in.
That is why the event is no longer recorded first.

**Signing up is open or by invitation** (`SigningUp`). The server sets it from `WEAVELING_SIGN_UP`.
When it is by invitation, a plain sign-up is refused, and the link is the only way in.
There is one exception, for the first account.
While nobody is an operator, a sign-up that brings the operator code from the server's log is let in.
The code is only checked in that case.
It makes that account the operator once its address is confirmed.
Meanwhile, `/api/service/mode` reports `operator_wanted`.
`POST /accounts/by-invitation` takes the link's token, a name and a password.
It creates the account as confirmed and opens its session right away.
It answers the same way signing in does.
This is because the person who followed the link does not need to know which address it was sent to.

**An invitation's ticket goes with what it lets in.**
The invitations feature tells accounts about each invitation it issues.
Accounts keeps a ticket for it, and stores its token only as a digest.
Taking an invitation up or turning it down takes the ticket and appends to the account in one [transaction](../../libraries/transacting/README.md).
So if the append fails, the ticket can be used again.
When a ticket arrives for an address that already has an account, the ticket is forgotten and the account declines the invitation, in the same transaction.
So if the decline fails, it is made again when the issue message is delivered again.
The ticket store's writes take that transaction; its reads do not.

**Sessions end by comparison, not by sweeping.**
An account records the moment its sessions last stopped counting, `sessions_since`.
A new password or a lock sets it.
Any session begun before that moment is over, wherever it is stored.
*A session is dated from when signing in began (2026-11-02):* `begun` is taken before the password is read and checked, not after Argon2 has said yes.
So if a reset lands while the old password is being checked, the session is dated before the reset, and it is over like the other sessions.
A locked account is let in with no session at all.

**Nothing tells a stranger whether an address has an account.**
Signing in with an address nobody has does the same Argon2 work, and is refused with the same words.
Signing up with an address that is taken is answered like any other sign-up, and the address's owner gets a mail.
Throttles count per address, whether or not an account stands behind it.
A locked account is told that it is locked only once its password is right.

**The operator** is the account that entered the code the server logs at start, at a time when there was no operator.
An operator may make other accounts operators.
The last operator is always kept.
Operators list, lock and unlock accounts.
See [the instance](../../ROADMAP.md#milestone-17--accounts-members-and-roles) in the roadmap for the rest of slice 3.

**An invitation into a project in the trash waits.**
*2026-11-02, slice 6d.*
The listener `accounts-remember-trashed-projects` is run through [`inbox::Inboxed`](../../libraries/inbox/README.md).
It receives `project.trashed`, `project.restored` and `project.deleted`.
It stores in the accounts catalog whether a project is in the trash (`remember_trash`, `forget_trash`, `is_in_the_trash`, in the table `trashed_projects` on PostgreSQL and SQLite).
It stores this by the project's version, and only ever moves forward.
*Until 2026-11-10 this was kept beside the invitation tickets. The user asked to move it to the catalog, because it is a read model of another feature's messages, just like the projects' members.*
Once a project is deleted, its row is forgotten completely, as members do.
Three actions check, inside their transaction and before the ticket is taken, whether the invitation's project is in the trash:
taking up an invitation into a project by its link, taking it up by its id, and signing up by one.
If the project is in the trash, they refuse with `InvitationPaused`, 409 `invitation-paused`.
The ticket is left as it was, so the invitation works again once the project is restored.
Turning an invitation down is still allowed.
An invitation to the instance is never in any trash.
The sweep after the purge still withdraws the invitation, as before.
Accounts makes this decision, not invitations or members, because taking up appends to the account before either of them hears about it.

**Deleting an account waits thirty days.**
*Slice 9, 2026-11-09.*

How a deletion is asked for:

- The holder asks for it with the password (`ask_deletion`).
- Or the operator marks another account for deletion (`mark_for_deletion`).

The account is then `PendingDeletion` until a day thirty days later (`KEPT_BEFORE_DELETION`). In that step:

- Every session is ended.
- An operator stops being an operator in the same step. If the account is the last operator, the request is refused with `LastOperator`. If the account is kept after all, the operator role does not come back.

Signing in to such an account is not refused.
Once the password and the second step are answered, the session opens at `Stage::DeletionPending`.
That session can do nothing except keep the account or sign out.
The holder can keep an account they asked to delete, from that session (`keep_account`). They stay signed in under a new token.
The operator can keep any account (`keep_account_of`).
A deletion the operator asked for can only be called off by the operator; the holder is refused with `DeletionAskedByTheOperator`.

Asking for deletion and marking for deletion are both refused while the account is the last owner of a project that other people are in.
The refusal is `ProjectsToHandOver`, and it names those projects.
A project in the trash is not counted.

Accounts learns who is in which project from the members' own messages. It never asks members.
The listener `remember-project-members` receives `members.founded`, `joined`, `role.changed`, `left`, `removed` and `disbanded`.
The catalog stores, for each account, what it is in each project: owner or member.
It stores this by the members' version for each account (`remember_holding`, `remember_disbanded`, `projects_to_hand_over`, in the tables `project_holdings` and `disbanded_projects`).
A member who is gone is stored as gone.
So a change that arrives late, as a retried delivery may, never undoes a newer one.
**The guards are forgotten once nothing can come late:**
a gone member stores when it went (`gone_at`), and a disbanded project stores when it was disbanded (`laid_at`).
The chore `forget-old-account-tombstones` (`ForgetOldTombstones` over `AccountsCore::forget_tombstones_laid_before`) forgets the ones laid more than `TOMBSTONES_KEPT_FOR` ago.
A disbanded project is forgotten together with everything held in it.
This is eventually consistent: an owner made a moment ago may not be counted yet. The sweep makes up for that (slice 9d).

Asking for deletion puts the account into the accounts' own trash (`libraries/trash`) until its day.
Keeping the account takes it out again.
The hourly chore `sweep-deleted-accounts` sweeps the accounts whose day has come (`AccountsCore::sweep`):

- The account becomes `Deleted`, done as the system.
- Afterwards it holds no name and no address (`deleted@account.invalid`, "Deleted").
- It refuses every command (`Gone`).
- It is compacted at once: its snapshot is deleted and every earlier event is pruned, so only its id remains.
- In the same transaction, its address is released, so it is free to sign up again.
- In the same transaction, every secret store forgets the account completely: its password, its sessions at any stage, its codes, its second step and its keys' credentials (`forget_password_of`, `end_every_session_of`, `forget_codes_of`, `forget_second_factors_of`, `forget_credentials_of`).

The catalog forgets the account completely when it receives `account.deleted`.
The events are published: `account.deletion.asked`, with the day and who asked, and `account.deletion.called.off`.
Members uses them to freeze the account's roles, and later to sweep it.

**Over REST** (*9e-1, 2026-11-10*):

- `POST /accounts/current/deletion` with the password asks for one's own account to be deleted. It answers `DeletionDTO { until, by }` and clears the cookie, because every session of the account has ended. If asked again, it answers with the day of the first request. `ask_deletion` and `mark_for_deletion` also just return the `PendingDeletion` that already exists.
- `DELETE /accounts/current/deletion` keeps the account. It is called from the session that signing in opened while the deletion is pending. The routes behind signing in reject that session. So this route sits on the open router and reads the cookie itself, as the second step's routes do. It answers `SignedInDTO` under a new cookie.
- `GET /sessions/current` reports `deletion` while one is pending.
- The refusals are `password-wrong`, `projects-to-hand-over` (with the `projects`), `last-operator`, `no-deletion-pending` and `deletion-asked-by-the-operator`. They are worded in all five languages.
- The operator marks another account with `PUT /accounts/{account}/deletion`, which answers `DeletionDTO`.
- The operator calls off any deletion with `DELETE` on the same path. It answers `204`, also when no deletion is pending, because calling off nothing changes nothing.
- Both operator routes take `If-Match`, like the operator's other actions. A version the account has moved past is refused as `outdated`. An account that was already swept is not found.
- The operators' list shows each account's pending deletion, `AccountSummaryDTO.deletion`. The catalog stores it as `deletion_until` and `deletion_asked_by` (*9e-2, 2026-11-10*).

**An account can show a picture** (*slice 11a, 2026-11-14*).

- The picture is an image kept by the assets feature. The account only holds its id, `PictureId`. Because this feature does not know the assets feature, the id is only checked to look like an id: letters, digits and underscores, at most 64 of them. Whether the image exists and is the account's own is for the assets feature to tell.
- `PUT /accounts/current/picture` with `{ picture, drop_previous }` shows a picture. It answers `204`. Showing the picture already shown changes nothing. An id that does not look like one is refused with `422 picture-id-invalid`.
- `DELETE /accounts/current/picture` shows no picture any more. `?drop=true` asks for the image to be deleted too. Removing a picture the account does not show changes nothing.
- The events are `PictureChanged { picture, previous, drop_previous }` and `PictureRemoved { picture, drop }`, published as `account.picture.changed` and `account.picture.removed`. The assets feature deletes the image named by `previous` or `picture` when `drop_previous` or `drop` is set and nothing else uses it.
- `GET /sessions/current` reports the picture as `picture`. A deleted account shows none.

**Core owns each write's transaction.**
`AccountsCore` begins every transaction from the feature's `Transactions`.

- Signing up claims the address, begins the account and stores the password in one transaction. The Argon2 hash and the throttle happen before it, and the mail after it.
- Every command on an account that already exists runs through `transacting::retrying!`. It gets five tries after a lost race. None of these commands names a version.
- When taking up or turning down an invitation is retried, taking the ticket is retried with it. So a ticket taken in a lost try is given back.
- The list's `remember` takes the transaction too.

A transaction that fails is reported as `AccountsError::Transaction`. It is no longer passed off as an error of the secrets.

The listeners `catalogue-account` and `keep-invitation-tickets` are run through [`inbox::Inboxed`](../../libraries/inbox/README.md).
It begins the transaction, records the message as received in it, and finishes it.
If it loses a race, the delivery offers the message again.
Cataloguing, and keeping, forgetting or dropping a ticket, all join that transaction.
So when a ticket arrives for an address with an account, the decline for that account happens in the same transaction as recording the message.
If that decline fails, the message stays unrecorded and is processed again.
`forget-old-account-hearings` forgets the old records of received messages. `AssembledAccounts` hands it out as part of `chores`.

Verifying closes its code in the transaction of its event.
Resetting a password stores the new password and closes its code in the transaction of its event.
A try at a code is counted first, in a transaction of its own, so a failure after it never gives the try back.
*Closing takes the code (2026-11-02):* `close` is the first thing done in the writing transaction.
It deletes the pending code only if it is still the code that was checked, and it reports whether it did.
So if two resets or two confirmations with one code both got past the check, the code is used only once. The second one is refused as a wrong code.
And if a new code was sent in the meantime, the old one does not close it.
Becoming an operator, being appointed and being dismissed write the event and the operators' table in one transaction.
The operator code is also taken in it.
So the operator status in the two places can never drift apart.
Opening a session, ending a session and issuing a code each happen in a transaction of its own.

**One way in: `AccountsCore`.**
Signing up, verifying, signing in and out, resetting a password, becoming an operator and cataloguing all go through one struct.
Locking, unlocking, listing, appointing and dismissing check whether the agent is an operator. They check it there and nowhere else.
The struct does the work itself.
Its use cases are kept one per file under `core/src/accounts/`: signing up, invitations, codes, sessions and operating. Each is an `impl` block of the same struct.
What they share is in `accounts.rs`.
There is no service behind it, so a reader of a use case sees all of it.
Errors are one enum, `AccountsError`.

**Crates:** `core` (`AccountsCore`, aggregate, secret stores, catalog port, service, tokens and codes) · `contract` (DTOs, refusals and routing keys, shared with the client) · `adapters/store` (stored-event codec) · `adapters/secrets` (the secret stores in memory, on PostgreSQL and on SQLite) · `adapters/catalog` (the list of accounts) · `adapters/messaging` (event publisher, catalog projector) · `adapters/rest` (routes, the gate, the cookie) · `wiring` · `tests`.
