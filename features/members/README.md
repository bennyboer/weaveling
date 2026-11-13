# members

Who may work in a project, and in which role.

**`Members` is an aggregate, one per project.**
It is stored under the project's own id.
So if it is founded twice for the same project, the store refuses the second one; no lookup is needed first.
Its events:

- `Founded` with its owner, the account that started the project.
- `Joined`: another account joins in a role.
- `RoleChanged`: a member's role changes.
- `Left` or `Removed`: a member is gone. The two are kept apart for the history.

The members are listed in the order they joined.
When the project is deleted, its members are `Disbanded`.
Everyone goes at once.
Nothing is let in afterwards: not a founding that arrives late, not a join, not a role change.
If the disbanding arrives first, the members begin already disbanded.
A snapshot keeps the fact that they were disbanded.
The listener `disband-members-of-deleted-project` receives `project.deleted`.
It has `MembersCore::disband` disband the project's members, as the system.
If the members were never founded, they begin as disbanded.
Once the list has received the disbanding, nobody may read the project.
So its owners can no longer list its members or invite into it.
If an invitation into it is taken up, the join is not carried out.

**Three roles:**

- an **Owner** does everything, including inviting, changing roles and deleting the project;
- an **Author** reads and writes;
- a **Reader** only reads.

What each role may do is decided by the `Authorizer`, not by the aggregate.
The aggregate stores who is in which role.
It also enforces the one rule that is about the members themselves.

**A project keeps at least one owner.**
The last owner cannot leave, cannot be removed, and cannot step down.
Either someone else is made an owner first, or the project is deleted.

**Repeating a change changes nothing.**
These produce no event:

- joining in the role one already has,
- changing a role to the role it already is,
- leaving, or removing someone who is already gone.

This is because invitation messages are delivered at least once.
Joining in a different role than the one a member has is refused. That would be a change of role.

**Who is in which project, and in which role, is a read model** (`MembershipCatalog`).
It stores each project's members as a whole, `ProjectMembers`, at their latest version.
The `Authorizer` asks it for one account's role in one project.
The projects list asks it for every project an account is in, `memberships_of`, ordered by project.
A newer list replaces the older one completely, so a member who left is gone with it.
An older list that arrives late changes nothing.
On PostgreSQL and SQLite, a project's version is claimed first, in `project_member_lists`.
Only a claim that succeeds replaces the project's rows in `memberships`.
All of this happens in one transaction.
Ids sort bytewise, so the order of an account's projects does not depend on the database's locale.
**Beside them it keeps who the accounts are.**
It stores a `Person` per account: the name and address, as accounts last published them (`remember_person`).
`people` returns the ones asked for that it knows, by account.
So the members can be listed by name.
On PostgreSQL and SQLite this is stored in `people`.
The listener `remember-who-signed-up` receives `account.signed.up` and `account.signed.up.by.invitation`.
They carry the name and address under `personal`.
The listener passes them to `MembersCore::remember_person`.
Accounts are never asked.

**An account that asked to be deleted is leaving, and keeps its role until it goes.**
The listener `remember-who-is-leaving` receives `account.deletion.asked` and `account.deletion.called.off`.
It stores, by the account's version, until when the account is leaving, or that it is kept after all (`remember_leaving`, `leaving`, in `leaving_accounts` on PostgreSQL and SQLite).
An older message that arrives after a newer one changes nothing.
Changing a leaving account's role is refused with `MembersCoreError::Leaving`, 409 `member-leaving`.
So nobody hands a book to an account that is about to go.
A leaving account can still be removed.
The members list shows until when each leaving member stays, `leaving_until`.

**A deleted account is removed.**
The listener `let-go-of-deleted-account` receives `account.deleted`.
It removes the account from every project it is in (`MembersCommand::LetGo`, which only the system may use):

- If it was the last owner, the longest-standing other member is made owner first. Then the account is removed.
- If nobody else was in the project, the members are `Abandoned` after the removal. This is published as `members.abandoned`, so that projects deletes the project. Abandoned members refuse everything except being disbanded.

The account's person is forgotten (`forget_person`).
Its leaving row is kept, with no date, as a guard against a message that arrives late.
An account that is no longer leaving, because it was kept or is gone, stores since when (`settled_at`).
The chore `forget-old-member-tombstones` forgets those rows `TOMBSTONES_KEPT_FOR` later, when no message about its leaving can be retried any more.

**Whoever starts a project owns it.**
The listener `found-members-of-started-project` receives `project.started`.
It founds the project's members with the account the message names as the actor.
A project started by no account, in local mode, has no members.
The `Authorizer` does not ask for members there.
Founding twice is not a failure: the store refuses the second begin as outdated.
That can happen, because a message is delivered at least once.

**Whoever takes up an invitation into a project joins it.**
The listener `join-by-accepted-invitation` receives `invitation.accepted`.
For an invitation into a project, it lets the account the message names as the actor join the project, in the role it was invited as.
The join is recorded as done by the system.
An invitation to the instance lets nobody join anything.
A member who is already in the project keeps their role, because changing it is an owner's job.
Joining again in the same role changes nothing, because a message is delivered at least once.

**The list of who may work where follows every change.**
Each change is published as `members.founded`, `members.joined`, `members.role.changed`, `members.left`, `members.removed` or `members.disbanded`.
The messages name accounts by id and roles by name. They never contain an address.
The listener `catalogue-members` receives them all.
It stores the project's current members, read from their events, not from what the message was about.

**The members answer the [`Authorizer`](../../libraries/authorizing)** on a server with accounts, as `MembersAuthorizer`.
It asks the list for the account's role:

- an owner may `Read`, `Write` and `Manage`;
- an author may `Read` and `Write`;
- a reader may `Read`;
- an account that is not in the project may do nothing.

The application acting on its own, `System`, may.
Nobody unsigned, `Local`, may not, because on a server nobody works unsigned.
If the list cannot be asked, that is a failure, not a "no".
The authorizer reads the list, not the aggregate, because it answers every request.
A role that was changed or taken away counts once the list has received the change, which takes moments.

**A project in the trash is read and written by nobody.**
The listener `members-remember-trashed-projects` is run through [`inbox::Inboxed`](../../libraries/inbox/README.md).
It receives `project.trashed` and `project.restored`.
It stores, by the project's version, whether the project is in the trash (`remember_trash`, `is_in_the_trash`, in `project_trash` on PostgreSQL and SQLite).
An older trashing that arrives after a newer restoring changes nothing.
For a project in the trash, `MembersAuthorizer` lets no account `Read`, `Write` or `Manage`.
It lets the project's owners `Restore`, so they can restore it and see it in the trash list.
*`Restore` has been an act of its own since 2026-11-02. The user asked whether an owner could still change a trashed project. The project already refused changes to its details. But `Manage` would have let an owner change its members' roles, remove them and invite into it.*
The authorizer also leaves a trashed project out of `readable_by`.
`System` still may act on it, because the purge deletes the project as the system.
Once the listener's work has committed, it reports `access_changed`, so open sockets ask again and lose access.

*This is an accepted gap, not a bug. Decided on 2026-11-02: the user chose members receiving the trash messages over an authorizer that wraps every mode.*
Without accounts, the authorizer is `AllowingEverything`, which receives no trash messages.
There, a trashed project is still hidden by the projects core: it is not listed and cannot be opened.
Only a stale tab's live socket, or a direct call to the API, can still write into it.
Nothing is lost and nothing becomes inconsistent: what is written is restored or purged together with the project.
And with one person on one machine, there is nobody to keep out.

*And forgotten once deleted (2026-11-02, at the user's request):*
the same listener receives `project.deleted` and forgets the project's row completely, `forget_trash`.
So the table holds only what is in the trash.
An old trashing could be delivered again after the deletion, for example a dead letter retried by hand.
It would write a row again.
That is harmless, because the project is gone and nobody is left in it. It is not worth a tombstone.

**Whoever is let in or out is told on the members' own channel.**
That is the topic `members` with `mine`, on the one socket `/api/live` (`MembersTopic`).
Every change to members is broadcast under `memberships`.
Each follower receives only the changes that name them:

- `FOUNDED` names its owner;
- `JOINED`, `ROLE_CHANGED`, `LEFT` and `REMOVED` name the account.

This happens whether or not the list has received the change yet.
So nobody has to wait for the list to learn that they are in.

**A project's own members can be followed there too**, with the topic `members` and a `project`.
Whoever may read the project receives every change to its members, filtered to that project.
`Read` is asked of the `Authorizer` when subscribing, and again on every change of access.
So someone who is removed has the subscription ended with `access-lost`.
The members page and the share dialog follow this topic and ask the list again.

**`GET /members/mine`** returns one's own memberships, `[{ project, role }]` (`MembershipDTO`), from the list (`memberships_of`).
Someone who is not an account gets nothing.
The client keeps these as its roles.
It asks again on every change it receives under `mine`, and offers only what a role may do.
Nobody unsigned receives anything.
A project's list follows it, beside the projects' own channel.

Being built in [slice 4](../../ROADMAP.md#milestone-17--accounts-members-and-roles). What stands so far:

- the domain,
- the stored-event codec,
- the read model in memory, on PostgreSQL and on SQLite,
- the messages and both listeners.

They are wired and assembled into a server with accounts.
There, whoever starts a project shows up as its owner in the list within moments.
The server holds its `Authorizer`, but nobody asks it yet.
The features that ask it on every route and socket follow.
The feature has no routes yet.

**One way in: `MembersCore`.**
Three things go through one struct:

- founding a started project's members, as the system,
- cataloguing them,
- deciding who receives a change on the members' channel (only the accounts the change names).

`MembersAuthorizer` stays the members' answer to the `Authorizer` port that every other core asks.
Errors are one enum, `MembersCoreError`, because `MembersError` is the aggregate's error.

**Changing who is in a project.**

- `GET /api/members?project=` lists a project's members and their roles to anyone who may read the project. Each member is shown by name, if members received their sign-up. The address is shown only to those who may manage the project, its owners. So authors and readers learn no address they were not given.
- `PATCH /api/members/{project}/accounts/{account}` with a role changes that member's role.
- `DELETE` on the same address removes the member.
- Both are an owner's job (`Manage`).
- `DELETE` on one's own account means leaving. That needs no permission, only being a member.
- Someone who is not a member gets "not there" for the project.

The refusals:

- A project keeps at least one owner. So the last owner cannot step down or leave (409 `last-owner`).
- Changing the role of someone who is not a member is 404 `not-a-member`.
- Removing someone who is already gone changes nothing, because a removal may be received twice.
- A role nobody knows is 422 `role-unknown`.
- A reader or an author trying an owner's work is told so, 403 `not-an-owner`.
- Someone who may not read the project gets "not there".

**The members themselves decide who manages them.**
Changing a role and removing check the members as they stand when the command is decided.
If the acting account is not an owner at that moment, they refuse with `NotAnOwner`.
So an owner who was removed or stepped down a moment ago cannot act on a list that has not received that change yet.
The version the command was decided at covers the race.
The authorizer's answer stays the quick check for the HTTP refusal.
The system, which acts for no member, may do these changes.
(*From the integrity audit, 2026-11-02.*)
Inviting into a project and withdrawing still ask only the authorizer.
That is because the invitation is not part of the members' aggregate and cannot share its transaction.
Each change answers with the members as they are after the change.
Joining is done by the system, carrying out an invitation that was taken up.

**Core owns each write's transaction.**
`MembersCore` begins every write's transaction, and `MembersService` carries out the command in it.
A change of role, a removal and leaving run through `transacting::retrying!`.
They get five tries after a lost race, because none of them names a version.
**Every listener is run through its inbox.**
The five listeners are `KeptHandler`s run through [`inbox::Inboxed`](../../libraries/inbox/README.md).
It begins the transaction and asks the feature's inbox whether the message was received before.
It then hands the transaction to the listener and finishes it.
If it loses a race, the delivery offers the message again.
Founding, joining, disbanding, cataloguing and storing a person take the transaction as `within` and join it.
So recording the message commits together with its effects, or nothing commits.
A founding that was already done, or a join that is refused, is not a failure.
Access is reported as changed in an `after_commit`, only once the list has committed.
The chore `forget-old-member-hearings` forgets the old records of received messages, through `HeardMessages`.

**Crates:** `core` (`MembersCore`, aggregate, read-model port, service, authorizer) · `contract` (routing keys, event DTO) · `adapters/store` (stored-event codec) · `adapters/catalog` (the read model in memory, on PostgreSQL and on SQLite) · `adapters/messaging` (event publisher and listeners) · `adapters/rest` (the members' routes and the live channel) · `wiring` (the feature, and the `Authorizer` it hands the server). `tests` (the routes, against the real authorizer over the real list). The listeners are tested in their adapter. The whole way, from starting a project, to its owner in the list, to the list being served, is tested by the server's journeys on PostgreSQL and on SQLite.
