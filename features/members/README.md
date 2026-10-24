# members

Who may work in a project, and as what.

**`Members` is an aggregate, one per project,** kept under the project's own id, so founding it twice for the same project is refused by the store rather than by a lookup first. It is `Founded` with its owner, whoever started the project; others `Joined` in a role; a role is `RoleChanged`; a member `Left` or was `Removed`, told apart for the history. The members are listed in the order they joined.

**Three roles:** an **Owner** does everything, inviting, changing roles and deleting the project among it; an **Author** reads and writes; a **Reader** only reads. What each role may do is the `Authorizer`'s to decide, not the aggregate's: the aggregate keeps who is in which role and the one rule that is about the members themselves.

**A project keeps at least one owner.** The last owner neither leaves, nor is removed, nor steps down; someone else is made an owner first, or the project is deleted.

**Told again, nothing changes:** joining in the role one has, changing a role to what it is, and leaving or removing someone gone already are no events, since invitations are heard at least once. Joining in another role than one has is refused; that is a change of role.

Being built in [slice 4](../../ROADMAP.md#milestone-17--accounts-members-and-roles): the domain stands. The store, the read model of who is in which project, founding on `project.created` and the `Authorizer` follow.

**Crates:** `core` (aggregate).
