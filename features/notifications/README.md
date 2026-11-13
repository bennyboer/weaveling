# notifications

This feature keeps the news for an account: what happened to it, perhaps while it was away. A notice is kept until the account puts it away.

**A `Notice` is an aggregate, one per thing told.** It is for one account and about one thing (`About`). It is `Noted` and may be `Dismissed`. Dismissing it again creates no event. It is event-sourced like every feature. So dismissing is recorded, and old notices can be retired the same way invitations are.

**The first thing told is a changed role.** The listener `note-role-changed` receives `members.role.changed`. It notes `About::RoleChanged` for the account that the message names. The notice holds the project, the account's new role, and who changed it. That is the account that acted, or none if the system did. If the role changes while the account is online, the client shows it at once, from the members' own channel. If the role changes while the account is away, the account finds the notice here when it comes back.

**Noted once, however often it is heard.** Every listener of the feature is a `KeptHandler` that receives its messages through [`inbox::Inboxed`](../../libraries/inbox/README.md). The inbox begins the transaction and checks with the feature's inbox whether the message was received before (`heard_before`). Then it hands the transaction to the handler. The handler passes it on to the core as `within`. `note`, `catalogue`, `remember_person`, `remember_project_name` and `lay_project_tombstone` all join this transaction. So a message delivered twice notes only one notice. If handling a message fails, the message is not recorded as received, so its next delivery takes it in.

**Each account's open notices are listed,** in a read model, `NoticeCatalog`. The listener `catalogue-notice` keeps it up to date from every `notice.noted` and `notice.dismissed`. It stores a `NoticeSummary` with the notice's latest version. So a change that arrives late never brings back a dismissed notice. `open_for` an account returns the account's notices that are not dismissed. They are sorted newest first, by when they were noted. (A notice stores `noted_at`, the moment it was noted, in its state and in its snapshot.) Anyone who is not an account has no notices.

**Only its account reads or dismisses it.** For anyone else, the system and a local agent included, the notice is not there, just like a notice that was never noted.

**In memory, on PostgreSQL and on SQLite.**

- The events are stored through `adapters/store`'s codec.
- The list is stored in `notices`. It has a column for each value the list is queried by, and for `project`. What the notice is about is stored as JSON, tagged by its kind, so a new kind needs no migration.
- The received messages are recorded in the inbox.

Each part keeps its own migration ledger, so the feature's schema is set up next to every other feature's schema. The list runs one conformance suite on all three backends.

**`GET /api/notifications`** returns one's own open notices, newest first. Each is a `NoticeDTO` with:

- its id,
- when it was noted, in RFC 3339,
- what it is about, as `AboutDTO`,
- `names`: the names of the people and the project it mentions, by their ids. So the client can say "Ada made you an author of Shared". A name that is not known is left out.

**`POST /api/notifications/{id}/dismissed`** puts one away. It answers `204`, however often it is called. It answers `404 not-found` for another account's notice, for a notice that was never noted, or for an id that is not valid. **Only with accounts:** the server assembles the feature next to members and invitations, behind signing in, with a database of its own, `notifications`. Without accounts, there is nothing to tell, and the routes do not exist.

**Names, as heard.**

- `notifications-remember-who-signed-up` receives both kinds of sign-up. It stores the account's name, but never its address, because no notice shows an address.
- `notifications-remember-project-names` receives when a project is started or renamed. It stores the project's name together with the project's version. A newer name replaces an older one. A name that arrives late changes nothing.

The listeners' names say which feature they belong to. Invitations receive the same messages with listeners of their own, and a listener's name is also the name of its queue. **A deleted project leaves a tombstone,** as in invitations. `notifications-remember-project-names` also receives `project.deleted`. Then it stores a tombstone in `project_tombstones`, with the time of the deletion, and deletes the project's name. So no name that arrives late brings the project back. A changed role in a project with a tombstone notes nothing, because `members.role.changed` may arrive after `project.deleted`. In that case, `note` returns `None`. The list leaves out every notice about such a project.

**Told live, on the one socket:** the topic `notifications` with `mine` follows a channel of the signed-in account's own (`NoticeChanges(account)`). Only an account can follow it, and only its own channel. `TellingNotices` wraps the list and sends a message to that channel in two cases:

- Each time the list takes in a change to one of the account's notices, noted or dismissed. The message is `of` the notice. So a frame arrives only once the list shows the change, and then the client asks again. The message is sent in an `after_commit` of the transaction the change was written in, so never for a change that was rolled back.
- Each time a project is deleted. Then every account with an open notice about the project gets a message `of` the project, because those notices leave the list. The list returns which accounts these are, with `accounts_noticed_about`.

**In the client, the bell** lists the notices in a group of their own, Roles, next to the invitations. (Invitations stay part of the invitations feature, because they are open questions rather than news.) A notice reads "Ada made you an author of Shared.", or "You are now a reader of a project." if the person or the project is not known by name. This exists in all five languages. Each notice is put away with Dismiss, quietly if it is already gone. The bell asks again on every frame of `notifications`.

**Old notices are forgotten, whole.** A notice stores when it was dismissed (`dismissed_at`, in its state, its snapshot and the list). The chore `forget-old-notices` runs every five minutes and handles a hundred notices at a time. It deletes every notice that:

- was dismissed thirty days ago or longer (`DISMISSED_KEPT_FOR`),
- was never dismissed, ninety days after it was noted (`UNDISMISSED_KEPT_FOR`),
- or is about a project with a tombstone.

Its events are deleted completely (`EventStore::forget`, nothing is kept, because a deleted notice serves no history). Its row in the list is deleted in the same transaction, so neither outlives the other.

`forget-old-notification-tombstones` deletes project tombstones one day after a change about the project can no longer be retried, as every feature does. `forget-old-notification-hearings` deletes the inbox records of received messages after the same time, because that message is not delivered again either.

**A deleted account's notices go with it.** The listener `forget-notices-of-deleted-account` receives `account.deleted`. It has `NotificationsCore::forget_account` delete every notice the list names for the account, dismissed or not (`notices_of`). Each notice is deleted completely, like an old one: its events, and its row with a tombstone. The account's name is deleted too (`forget_person`). All of this happens in the transaction of the received message. A notice that was noted but not yet listed at that moment is missed. It is deleted later, like an old one.

**Core owns each write's transaction.** `NotificationsCore` has no service behind it. It writes through the event-sourcing service and through the list itself. Noting joins the caller's transaction, or begins one. Dismissing runs through `transacting::retrying!`, with five tries after a lost race. Every write to the list begins its own transaction: a summary, a name, a tombstone, or a deletion. The list's writes use that transaction, and the list's suite proves that what a dropped transaction wrote is gone.

**Crates:** `core` (`NotificationsCore`, `Notice`, `NoticeId`, `NoticeCatalog`) · `contract` (`NoticeEventDTO`, `AboutDTO`, the routing keys) · `adapters/catalog` (in memory, PostgreSQL, SQLite, and the conformance suite every list runs) · `adapters/store` (the codec the event store keeps notices in) · `adapters/rest` (the two routes) · `adapters/messaging` (publishing, `NoteRoleChangedListener`, `CatalogueNoticeListener`) · `wiring` · `tests`.
