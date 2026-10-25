# authorizing

Who may do what in a project.

**`Authorizer` is a port** every feature asks before it reads, writes or manages anything of a project: `may(agent, act, project)`. Features know only this library; who is in which project as what belongs to the members feature, which answers it, so no feature asks another.

**Three acts.** `Read` a project and all it holds; `Write` in it, its scenes, ideas, boards and outline; `Manage` the project itself, its name and details, its members and invitations, and deleting it. Which role may which is the members feature's to say.

**What may one read, as a whole:** `readable_by(agent)` answers `Everything` or `Only` these projects, so a list of projects asks once, not once a project.

**In local mode one person does everything**, so `AllowingEverything` answers yes to anyone. On a server with accounts the members feature answers instead.

**A failure to ask is no answer.** `may` fails rather than guessing, so a route answers that something went wrong rather than letting anyone in or locking the owner out.
