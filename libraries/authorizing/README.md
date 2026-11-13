# authorizing

Who may do what in a project.

**`Authorizer` is a port.** Every feature asks it before it reads, writes or manages anything of a project: `may(agent, act, project)`. Features know only this library. Who is in which project, and in which role, belongs to the members feature, and that feature answers. So no feature asks another feature.

A feature asks before it does anything. A list asks only once:

```rust
use authorizing::{Act, Authorizer, Readable};

if !self.authorizer.may(agent, Act::Write, project.as_str()).await? {
    return Err(IdeasError::NotAnAuthor);
}

let projects = match self.authorizer.readable_by(agent).await? {
    Readable::Everything => self.catalog.all().await?,
    Readable::Only(projects) => self.catalog.among(&projects).await?,
};
```

**There are four acts.**

- `Read`: read a project and everything in it.
- `Write`: write in a project: its scenes, ideas, boards and outline.
- `Manage`: manage the project itself: its name and details, its members and invitations, and deleting it.
- `Restore`: restore the project from the trash. This is the only act still allowed on a project in the trash. So a trashed project can be brought back, while its members and invitations stay as they were.

Which role may do which act is decided by the members feature.

**What may someone read, as a whole?** `readable_by(agent)` returns either `Everything` or `Only` a set of projects. So a list of projects asks once, not once per project.

**In local mode one person does everything.** So `AllowingEverything` answers yes to anyone. On a server with accounts, the members feature answers instead.

**A failed check is not an answer.** `may` returns an error rather than guessing. So a route reports that something went wrong. It neither lets anyone in nor locks the owner out.

**Who may follow a project can change while they follow it.** `access_changed(project)` signals that the answers about a project may have changed. `watch_access(project)` returns an `AccessWatch`. Its `changed()` wakes up when the answers changed, or when the watch fell behind and cannot tell. Both work in-process, through `AccessChanges`. An authorizer whose answers never change, such as `AllowingEverything`, keeps the defaults: a watch that never wakes up. The members feature signals a change once its own list already shows it. So whoever asks again after waking up gets the answer from the current list.
