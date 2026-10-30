# notifications

What an account is told happened to it while it may have been away, kept until it is put away.

**A `Notice` is an aggregate, one per thing told,** for one account, about one thing (`About`). It is `Noted` and may be `Dismissed`; dismissing it again is no event. Event-sourced as every feature is, so that dismissing is recorded and old notices can retire the same way invitations do.

**The first thing told is a changed role.** The listener `note-role-changed` hears `members.role.changed` and notes `About::RoleChanged` for the account it names: the project, the role it is now, and who changed it, the account that acted, none when the system did. One changed while its account was online is told at once by the client from the members' own channel; one changed while it was away is found here on coming back.

**Noted once, however often it is heard.** A notice is claimed in a `Registry` under its cause, the members' event that told it (`members:project_1@4`), so a message heard twice notes one notice.

**Each account's open notices are listed,** a read model (`NoticeCatalog`) the listener `catalogue-notice` keeps from every `notice.noted` and `notice.dismissed`: a `NoticeSummary` by its latest version, so a change heard late never brings a dismissed notice back; `open_for` an account answers its notices not dismissed, newest first, by when they were noted (a notice keeps `noted_at`, the moment it was noted, in its state and its snapshot). Anyone but an account has none.

**Only its account reads or dismisses it.** Anyone else, the system and a local agent among them, finds it not there, as for a notice nobody noted.

*So far:* the feature on its own, in memory, no routes, not yet assembled into the server. Next: its stores on PostgreSQL and SQLite (5g-2b), the routes (5g-3), people's and projects' names, cleaned up when a project is deleted (5g-4), told live (5g-5), the bell (5g-6), and old notices retiring (5g-7).

**Crates:** `core` (`NotificationsCore`, `Notice`, `NoticeId`, `NoticeCatalog`) · `contract` (`NoticeEventDTO`, `AboutDTO`, the routing keys) · `adapters/catalog` (in memory, and the conformance suite every list runs) · `adapters/messaging` (publishing, `NoteRoleChangedListener`, `CatalogueNoticeListener`) · `wiring` · `tests`.
