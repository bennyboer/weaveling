# members

Who may work in a project, and as what.

**`Members` is an aggregate, one per project,** kept under the project's own id, so founding it twice for the same project is refused by the store rather than by a lookup first. It is `Founded` with its owner, whoever started the project; others `Joined` in a role; a role is `RoleChanged`; a member `Left` or was `Removed`, told apart for the history. The members are listed in the order they joined.

**Three roles:** an **Owner** does everything, inviting, changing roles and deleting the project among it; an **Author** reads and writes; a **Reader** only reads. What each role may do is the `Authorizer`'s to decide, not the aggregate's: the aggregate keeps who is in which role and the one rule that is about the members themselves.

**A project keeps at least one owner.** The last owner neither leaves, nor is removed, nor steps down; someone else is made an owner first, or the project is deleted.

**Told again, nothing changes:** joining in the role one has, changing a role to what it is, and leaving or removing someone gone already are no events, since invitations are heard at least once. Joining in another role than one has is refused; that is a change of role.

**Who is in which project as what is a read model** (`MembershipCatalog`), kept as each project's members whole, `ProjectMembers`, by their latest version: the `Authorizer` asks it for one account's role in one project, and the projects list for every project an account is in, its `memberships_of`, ordered by project. A newer list replaces the older whole, so whoever left is gone with it; an older one heard late changes nothing. On PostgreSQL and SQLite a project's version is claimed first, in `project_member_lists`, and only a claim that took replaces its rows in `memberships`, all in one transaction; ids sort bytewise, so the order of an account's projects does not hang on the database's locale.

Being built in [slice 4](../../ROADMAP.md#milestone-17--accounts-members-and-roles): the domain, the stored-event codec and the read model, in memory, on PostgreSQL and on SQLite, stand. Founding on `project.created` and the `Authorizer` follow.

**Crates:** `core` (aggregate, read-model port) · `adapters/store` (stored-event codec) · `adapters/catalog` (the read model in memory, on PostgreSQL and on SQLite).
