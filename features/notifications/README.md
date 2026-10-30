# notifications

What an account is told happened to it while it may have been away, kept until it is put away.

**A `Notice` is an aggregate, one per thing told,** for one account, about one thing (`About`). It is `Noted` and may be `Dismissed`; dismissing it again is no event. Event-sourced as every feature is, so that dismissing is recorded and old notices can retire the same way invitations do.

**The first thing told is a changed role.** The listener `note-role-changed` hears `members.role.changed` and notes `About::RoleChanged` for the account it names: the project, the role it is now, and who changed it, the account that acted, none when the system did. One changed while its account was online is told at once by the client from the members' own channel; one changed while it was away is found here on coming back.

**Noted once, however often it is heard.** A notice is claimed in a `Registry` under its cause, the members' event that told it (`members:project_1@4`), so a message heard twice notes one notice.

**Only its account reads or dismisses it.** Anyone else, the system and a local agent among them, finds it not there, as for a notice nobody noted.

*So far:* the feature on its own, in memory, no routes, not yet assembled into the server. Next: its stores and list (5g-2), the routes (5g-3), people's and projects' names, cleaned up when a project is deleted (5g-4), told live (5g-5), the bell (5g-6), and old notices retiring (5g-7).

**Crates:** `core` (`NotificationsCore`, `Notice`, `NoticeId`) · `contract` (`NoticeEventDTO`, `AboutDTO`, the routing keys) · `adapters/messaging` (publishing, `NoteRoleChangedListener`) · `wiring` · `tests`.
