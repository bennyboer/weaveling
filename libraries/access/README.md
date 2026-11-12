# access

Who may do what, as far as a feature has heard. Each feature that asks whether someone may read or change a project, or runs the instance, keeps its own answer in its own database, from what the members, projects and accounts features tell, rather than asking one feature for all at every request; so it depends on none of them at work, and hears a change some milliseconds after it is made.

**`AccessCatalog` is the port,** written in the caller's [transaction](../transacting/README.md) as a feature's listener takes a word in, and read on its own:

- `remember_role(project, account, version, role, at)`: an account's `Role` in a project, `Owner`, `Author` or `Reader`, or with none that it is a member no more; `Role::permits(act)` says which acts each allows.
- `remember_disbanded(project, version, at)`: the project's members disbanded, its project deleted, so nobody is a member of it whatever is heard of it after.
- `remember_trash(project, version, in_the_trash)`: whether the project is in the trash.
- `remember_operator(account, version, operator, at)`: whether the account runs the instance.
- `role_of`, `is_in_the_trash`, `projects_of` (neither disbanded nor in the trash), `share_a_project` and `operates` answer them.

**Every word comes with the version of what it is about,** the project's members, the project or the account, and the newest heard wins, as a message refused may be tried again after newer ones: a word heard late or twice changes nothing. What has ended, a membership, an operator's running of the instance, a project disbanded, stays as a tombstone with when it ended, so an older word heard after it does not bring it back; `forget_tombstones_laid_before(before, at_most)` lets them go, for each feature's tombstone chore, once nothing can be heard late any more.

**The library knows no feature's contract:** each feature maps the members', projects' and accounts' events onto the port in a listener of its own, near alike from feature to feature, so libraries stay below features without exception.

In memory so far, held to one conformance suite.
