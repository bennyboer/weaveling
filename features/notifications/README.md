# notifications

What an account is told happened to it while it may have been away, kept until it is put away.

**A `Notice` is an aggregate, one per thing told,** for one account, about one thing (`About`). It is `Noted` and may be `Dismissed`; dismissing it again is no event. Event-sourced as every feature is, so that dismissing is recorded and old notices can retire the same way invitations do.

**The first thing told is a changed role.** The listener `note-role-changed` hears `members.role.changed` and notes `About::RoleChanged` for the account it names: the project, the role it is now, and who changed it, the account that acted, none when the system did. One changed while its account was online is told at once by the client from the members' own channel; one changed while it was away is found here on coming back.

**Noted once, however often it is heard.** A notice is claimed in a `Registry` under its cause, the members' event that told it (`members:project_1@4`), so a message heard twice notes one notice.

**Each account's open notices are listed,** a read model (`NoticeCatalog`) the listener `catalogue-notice` keeps from every `notice.noted` and `notice.dismissed`: a `NoticeSummary` by its latest version, so a change heard late never brings a dismissed notice back; `open_for` an account answers its notices not dismissed, newest first, by when they were noted (a notice keeps `noted_at`, the moment it was noted, in its state and its snapshot). Anyone but an account has none.

**Only its account reads or dismisses it.** Anyone else, the system and a local agent among them, finds it not there, as for a notice nobody noted.

**In memory, on PostgreSQL and on SQLite.** The events go through `adapters/store`'s codec, the list into `notices`, a column for what it is asked by and `project`, what the notice is about as JSON tagged by its kind, so a new kind needs no migration, the claims into the `Registry`; each part keeps a ledger of its own, so the feature's schema lays down beside every other's. The list runs one conformance suite on all three.

**`GET /api/notifications`** answers one's own open notices, newest first (`NoticeDTO`: its id, when it was noted, RFC 3339, what it is about, `AboutDTO`, and `names`, the names of the people and the project it mentions, by their ids, so the client can say "Ada made you an author of Shared"; whatever is not known by name is left out); **`POST /api/notifications/{id}/dismissed`** puts one away, `204` however often, `404 not-found` for another's notice, one nobody noted or no id at all. **Only with accounts:** the server assembles the feature beside members and invitations, behind signing in, with a database of its own, `notifications`; without accounts there is nothing to be told and the routes are not there.

**Names, as heard.** `notifications-remember-who-signed-up` hears both kinds of sign-up and keeps the account's name, never its address, which no notice shows; `notifications-remember-project-names` hears a project started or renamed and keeps its name by the project's version, a newer one replacing an older, one heard late changing nothing. The listeners' names say whose they are, as invitations hear the same with listeners of their own, and a listener's name is its queue's. **A deleted project leaves a tombstone,** as in invitations: `notifications-remember-project-names` hears `project.deleted` too, lays `project_tombstones` with when it was deleted and forgets the project's name, so no name heard late brings it back. A role changed in a project with a tombstone notes nothing, as `members.role.changed` may be heard after `project.deleted`; the list leaves out every notice about it; `note` answers `None` then.

**Told live, on the one socket:** the topic `notifications` with `mine` follows, for the signed-in account, a channel of its own (`NoticeChanges(account)`); nobody but an account follows, and only its own. `TellingNotices`, around the list, tells that channel each time the list takes in a change to one of the account's notices, noted or dismissed, `of` the notice, so a frame comes only once the list shows it and the client asks again; and each time a project is deleted, every account with an open notice about it, `of` the project, as those notices leave the list. Whose notices those are the list answers, `accounts_noticed_about`.

**In the client, the bell** lists them in a group of their own, Roles, beside the invitations (which stay the invitations feature's, open questions rather than news): "Ada made you an author of Shared.", or "You are now a reader of a project." when who or which is not known by name, in all five languages; each put away by Dismiss, quietly if it is gone already; asked again on every frame of `notifications`.

*Next:* old notices retiring (5g-7).

**Crates:** `core` (`NotificationsCore`, `Notice`, `NoticeId`, `NoticeCatalog`) · `contract` (`NoticeEventDTO`, `AboutDTO`, the routing keys) · `adapters/catalog` (in memory, PostgreSQL, SQLite, and the conformance suite every list runs) · `adapters/store` (the codec the event store keeps notices in) · `adapters/rest` (the two routes) · `adapters/messaging` (publishing, `NoteRoleChangedListener`, `CatalogueNoticeListener`) · `wiring` · `tests`.
