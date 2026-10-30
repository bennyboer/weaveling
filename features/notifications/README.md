# notifications

What an account is told happened to it while it may have been away, kept until it is put away.

**A `Notice` is an aggregate, one per thing told,** for one account, about one thing (`About`). It is `Noted` and may be `Dismissed`; dismissing it again is no event. Event-sourced as every feature is, so that dismissing is recorded and old notices can retire the same way invitations do.

**The first thing told is a changed role.** The listener `note-role-changed` hears `members.role.changed` and notes `About::RoleChanged` for the account it names: the project, the role it is now, and who changed it, the account that acted, none when the system did. One changed while its account was online is told at once by the client from the members' own channel; one changed while it was away is found here on coming back.

**Noted once, however often it is heard.** A notice is claimed in a `Registry` under its cause, the members' event that told it (`members:project_1@4`), so a message heard twice notes one notice.

**Each account's open notices are listed,** a read model (`NoticeCatalog`) the listener `catalogue-notice` keeps from every `notice.noted` and `notice.dismissed`: a `NoticeSummary` by its latest version, so a change heard late never brings a dismissed notice back; `open_for` an account answers its notices not dismissed, newest first, by when they were noted (a notice keeps `noted_at`, the moment it was noted, in its state and its snapshot). Anyone but an account has none.

**Only its account reads or dismisses it.** Anyone else, the system and a local agent among them, finds it not there, as for a notice nobody noted.

**In memory, on PostgreSQL and on SQLite.** The events go through `adapters/store`'s codec, the list into `notices`, a column for what it is asked by and `project`, what the notice is about as JSON tagged by its kind, so a new kind needs no migration, the claims into the `Registry`; each part keeps a ledger of its own, so the feature's schema lays down beside every other's. The list runs one conformance suite on all three.

*So far:* the feature on its own, no routes, not yet assembled into the server. Next: the routes (5g-3), people's and projects' names, cleaned up when a project is deleted (5g-4), told live (5g-5), the bell (5g-6), and old notices retiring (5g-7).

**Crates:** `core` (`NotificationsCore`, `Notice`, `NoticeId`, `NoticeCatalog`) · `contract` (`NoticeEventDTO`, `AboutDTO`, the routing keys) · `adapters/catalog` (in memory, PostgreSQL, SQLite, and the conformance suite every list runs) · `adapters/store` (the codec the event store keeps notices in) · `adapters/messaging` (publishing, `NoteRoleChangedListener`, `CatalogueNoticeListener`) · `wiring` · `tests`.
