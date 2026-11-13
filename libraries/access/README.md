# access

Who may do what, based on the events a feature has received so far.

Some features need to know whether someone may read or change a project, or whether someone is an operator of the instance. Each such feature keeps its own copy of the answer in its own database. It builds that copy from the events of the members, projects and accounts features. It does not ask one central feature on every request. So at runtime it depends on none of those features. The price is that it learns of a change a few milliseconds after the change is made.

**`AccessCatalog` is the port.** It is written in the caller's [transaction](../transacting/README.md), when a feature's listener receives a message. It is read on its own, outside any transaction.

- `remember_role(project, account, version, role, at)`: stores an account's `Role` in a project: `Owner`, `Author` or `Reader`. With no role, it stores that the account is no longer a member. `Role::permits(act)` says which acts each role allows.
- `remember_disbanded(project, version, at)`: stores that the project's members were disbanded, because the project was deleted. From then on nobody is a member of it, whatever messages about it arrive later.
- `remember_trash(project, version, in_the_trash)`: stores whether the project is in the trash.
- `remember_operator(account, version, operator, at)`: stores whether the account is an operator of the instance.
- These are answered by `role_of`, `is_in_the_trash`, `projects_of` (projects that are neither disbanded nor in the trash), `share_a_project`, `operates` and `owners_of`. `owners_of` returns a project's owners by id. It returns none once the project is disbanded.

**Every message carries the version of the thing it is about.** That thing is the project's members, the project, or the account. The newest version wins. This matters because a refused message may be retried after newer ones were already handled. So a message that arrives late, or arrives twice, changes nothing.

Some things end: a membership, an account's operator role, a project that was disbanded. Each of these stays as a tombstone that records when it ended. So an older message that arrives afterwards does not bring it back. `forget_tombstones_laid_before(before, at_most)` deletes old tombstones. Each feature's tombstone chore calls it once no late message can arrive any more.

**`KeptAccess` is the `Authorizer` built on this catalog.** It answers the way the members feature itself would:

- The system may do everything.
- Someone who is not signed in may do nothing.
- An account may do what its role permits.
- In a project that is in the trash, an account may only restore it.
- An account can read the projects it is a member of.
- An account is an operator if the last message about it said so (`Authorizer::operates`). `AllowingEverything` answers yes to this, so without accounts the single user is the operator.

A feature's listener updates the catalog through `remember_role`, `remember_disbanded`, `remember_trash` and `remember_operator`, in the listener's transaction. Once that transaction is committed, everyone who follows the project live is notified that its access changed. So a live socket closes for someone who is no longer allowed. `share_a_project` answers whether two accounts are both members of some project. The assets feature asks this before it shows one account's own assets to another account.

**The library knows no feature's contract.** This way libraries always stay below features, without exception. Instead, the contracts that publish the events provide the listeners, each as a macro:

- `members_contract::access_roles_listener!`
- `projects_contract::access_trash_listener!`
- `accounts_contract::access_operators_listener!`

Each is one line in the feature's messaging crate. The line names the listener, the core, the listener's name, and the error to report when a message is refused. The listeners pass what they receive to the core through two traits that each core implements: `HearsAccess` (`remember_role`, `remember_disbanded`, `remember_trash`) and `HearsOperators` (`remember_operator`). So the compiler makes every feature handle these messages the same way. A macro adds nothing to a contract's own dependencies, so the contracts can still be used in the browser.

**Each feature creates the tables in its own database.** This works like [`belonging`](../belonging/README.md): it is a shared mechanism, not shared data. The tables are `access_roles`, `access_disbanded`, `access_trash` and `access_operators`. They have their own migration ledger, `_sqlx_migrations_access`. There are implementations in memory, on PostgreSQL and on SQLite, and all three must pass one conformance suite. Updates are written in the caller's transaction. Questions are read on the feature's own pool, so the `Authorizer` built on it can answer without a transaction.
