# Conventions

This file describes how the code in this repository is written. The goal is that a reviewer never has to explain the same thing twice. [ARCHITECTURE.md](./ARCHITECTURE.md) says what the parts are and why they exist. This file says how they are spelled.

## Naming

### The register depends on the layer

Weaveling's domain language is chosen on purpose. For example, an idea is *captured*, a project is *started*, and an outline *attaches* a scene to a section. That language belongs in the domain: in aggregates, commands, events, errors, and in the service methods that an author's actions map onto. So `capture`, `retitle` and `discard` are right there. `create`, `update` and `delete` would be worse.

**Below the domain, that language is the wrong register.** Adapters, wiring and libraries are plumbing. Plumbing needs a plain technical name that says what the thing produces or does. An evocative name there adds no meaning. It only hides the type.

| Layer | Register | Example |
| --- | --- | --- |
| `core` — aggregate, command, event, error, service | The domain's own words | `IdeaCommand::Capture`, `ProjectEvent::Started` |
| `adapters/*`, `wiring`, `libraries/*` | Plain and technical | `to_dto`, `to_response`, `CatalogueProjectsListener` |
| Test function names | A sentence that states the claim | `a_notification_delivers_long_before_the_next_poll_would` |
| Test helpers | Read well at the call site, and start with an article | `a_title()`, `an_author()`, `at(1_000)` |

### A converter is named for what it returns

Name it `to_<thing>`. Do not name it for what the caller happens to do with the result. Do not name it with a participle that describes the result.

```rust
fn to_dto(summary: &ProjectSummary) -> ProjectDTO     // yes
fn to_response(status, id, standing) -> Response      // yes
fn to_summary(row: &PgRow) -> Result<ProjectSummary>  // yes

fn listed(summary: &ProjectSummary) -> ProjectDTO     // no — names the caller's use
fn reported(status, id, standing) -> Response         // no — veils what it produces
fn summarised(row: &PgRow) -> Result<ProjectSummary>  // no — participle for a conversion
```

By convention, Rust uses the `to_*` names for methods. But these converters are free functions that get passed to `map`. There, `found.iter().map(to_dto)` reads exactly right.

**If two converters in one file both want the same name, that is a signal to keep only one.** `listed` existed only because `to_dto` was already taken by another converter next to it. That other converter was used only once. Inlining it freed the name. After that, each adapter had exactly one converter.

### A type is named for what it is, spelled out

Do not use bare abstract nouns. `Publishing` was once the only bare word in an export list. The other names in that list were `UnreadableProjectEvent`, `CatalogueProjectsListener` and `ProjectSummary`. `Publishing` said less than `ProjectEventPublisher`, which became its new name.

The name shapes that every feature repeats:

- `<Thing>Catalog`, `<Thing>Summary`, `<Thing>CatalogProjector`
- `<Thing>Event`, `<Thing>Command`, `<Thing>Error`, `<Thing>EventDTO`, `Stored<Thing>Event`
- `Unreadable<Thing>Event`
- `<Things>Core` is the feature's only way in, and `<Things>Error` holds its errors. Examples: `ProjectsCore`, `ProjectsError`. Both are plural, like the feature. Use a name like `MembersCoreError` only where the aggregate's `<Thing>Error` already uses the plain name.
- `<Thing>Service` sits behind the core and is not exported.
- `InMemory<Thing>Catalog`, `Postgres<Thing>Catalog`

In the client, a component is named for its role:

- A **`<Thing>Page`** is what a route mounts. It reads the address, puts the masthead above, and passes plain values down. Examples: `BoardPage`, `IdeasPage`, `WritePage`. Each one lives in `app/`, in a file named after it.
- A **`<Thing>View`** is what a page shows. It takes plain values instead of the router, so it can be mounted in other places too. Examples: `BoardView`, `OutlineView`, `WriteView`. Each one lives in its feature's module.

Do not use names like `OneBoard` or `TheBoard`. An article says nothing about which of the two kinds a component is.

In the client, a component that loads and changes things keeps that logic in a **`<Component>State`**. The state lives next to the component and carries the component's full name. Examples: `BoardViewState` for `BoardView`, `ScenePageState` for `ScenePage`, `InspectorState` for `inspector`. The views take it as a parameter named `state`. Do not use `BoardState`: it reads as the state of a board, not as the state of the view that shows a board.

When a state does several jobs, it is made of parts, and each part is named for its job. For example, `WriteViewState` is made of `Structure`, `Walk` and `Beginnings`. The `Problems` that these parts report into is held above all of them. Callers say which part they mean, as in `state.walk.leave(…)`.

A state is named for the view, not for how the view got its data. For example, the inspector's state is the same in both cases: when the idea came from the address, and when the inspector is docked on the board and the idea came from a selection.

Type names are singular and built on the aggregate's own name: `ProjectSummary` and `ProjectEventDTO`, not `ProjectsSummary`. The crate name is plural (`projects_messaging`). The types inside it are not.

A listener is named for what it does. Its name is a verb phrase that ends in **`Listener`**: `CatalogueProjectsListener`, `DeleteOnProjectDeletedListener`, `BroadcastChangesListener`. This way, the wiring reads as a list of what happens. It also makes a listener recognisable as a listener wherever it is used. Do not use a name like `ProjectCatalogProjector`. A role noun says what a thing is, not what it does. Two role nouns in a row say even less.

### A contract crate has three suffixes and no fourth

Everything in a `contract` crate crosses a process boundary. The suffix says in which direction:

- **`*Request`** — a REST body coming in: `CaptureIdeaRequest`, `OpenBoardRequest`.
- **`*Response`** — a REST body going out that is not a resource: `AddedSectionResponse`.
- **`*DTO`** — everything else on the wire: resources (`IdeaDTO`), value objects (`SpotDTO`), event payloads (`IdeaEventDTO`) and message payloads (`MoreToSweepDTO`).

`DTO` is mostly there for *disambiguation*. It tells names apart, as in `Idea`/`IdeaDTO` and `IdeaEvent`/`IdeaEventDTO`. But it also marks a wire shape when no domain type has the same name. The reason: one unmarked type among a dozen marked ones looks like an oversight, not like a decision.

**Use one suffix, never a pile of them.** `MoreToSweepMsgPayloadDTO` says *data crossing a boundary* three times. Also, an `Ideas` inside a type name stutters against the crate name `ideas_contract`. The rule is to spell out what a thing **is**. Do not stack up what it **is made of**.

### Reading something out of something else

Use `<thing>_in(<source>)` when a value is taken out of a message or an envelope: `project_in(message)`, `idea_in(message)`, `event_in(message)`, `published_in(message)`. This is different from `to_*`. `to_*` converts a whole value. `<thing>_in` picks one value out.

### Where participles do belong

Participles belong in predicates and states, not in conversions: `is_deleted`, `is_snapshot`, `is_publishable`, `Delivery::Kept`. A participle answers *what is true of this*. It never answers *what does this return*.

### Name the mechanism, not the feeling

This is the same trap in a different shape. Here, a word is chosen because it evokes the right *idea*, not because it names what happens. Such a word reads well the first time. The second time, it tells a reader nothing.

- `Nudges` / `outbox.nudges()` was renamed to **`Notifications`** / `outbox.notifications()`. Both backends already call it notify: PostgreSQL has `pg_notify` and `LISTEN`, and tokio has `Notify`. So "nudge" was a third word for a thing that already had two good ones.
- `announce` was renamed to **`enqueue`**. The in-memory outbox already had `enqueue`, so the two were one operation with two names in one crate. What the operation does is put a row in a queue.

**Prose is the exception, and deliberately so.** A test name and an assertion message may keep the domain word. For example, "a refused append announces nothing" says what an outbox row *means* to a reader. `enqueue` says what the code *does*. Meaning goes in the prose, and mechanism goes in the code. The two may differ, as long as each stays in its own place.

## Comments

**There are none.** The code says what it does. The tests say what it is for. The one exception is an **assertion message**. It carries the *why*, which a reader could not work out from the code alone:

```rust
assert!(
    until(|| heard.how_many() == 1).await,
    "polling is five minutes away, so only the notification can have woken it"
);
```

An assertion message says why the expectation holds, or what breaks if it does not. It never restates the assertion in prose.

**The other exception is a `// TODO`, and every stopgap needs one.** Sometimes a slice leaves something unfinished on purpose, because a later slice will finish it. Then a `// TODO` goes at the spot where the next reader will run into it. It says:

- what is missing,
- why it is like this for now,
- and which slice replaces it.

```rust
// TODO 5b-2: project invitations keep no ticket yet, so an account is never declined for one; accounts will keep their tickets with the scope and decline for the instance alone
InvitationEventDTO::Issued { scope: ScopeDTO::Project { .. }, .. } => Ok(()),
```

The ROADMAP says the same in its notes. But a note in the ROADMAP is not read where the code is. A `// TODO` is removed in the slice it names.

**The third exception is a test's steps**, as in Spock. See [Tests](#tests).

## Tests

- The test name is the claim, written as a sentence: `a_deleted_project_refuses_everything`, `a_batch_that_collides_halfway_writes_none_of_itself`.
- Every test, in Rust and in the browser, can be read through its steps alone, without its code. The steps are `// given: …`, `// when: …` and `// then: …`. Each step is followed by the code it describes. `// and: …` continues the step above it. A `then` may be followed by another `when` and `then`. A test that only checks something has a `then` alone. Where there is nothing to set up, it uses `expect:` instead. Each step is short and lower case. It says only what stays true of the behaviour for as long as that behaviour exists. It does not say how the behaviour is done today. It does not say which other feature the behaviour relies on, unless the claim is about that feature.

  ```rust
  // given: an accepted invitation
  let accepted = invited(&wired).await;
  // and: a withdrawn one
  let withdrawn = a_withdrawn_invitation(&wired).await;

  // when: a taking up of each is heard
  for id in [&accepted, &withdrawn] { /* … */ }

  // then: the accepted one is accepted once
  assert_eq!(events_of(&wired, &accepted).await.len(), 2);
  ```
- Helpers are named for the step they serve, so the code reads like its steps:
  - A `given` helper names the state it sets up (`an_open_invitation`, `a_project(&service, "Tapestry")`).
  - A `when` helper names the act (`hearing_it_taken_up`).
  - A `then` helper names what it reads back (`events_of`).

  Helpers that make a value start with an article, so call sites read as English: `an_author()`, `a_workbench()`.
- The assembled world of a test is a **`Wired`**, built by **`a_workbench()`**. This is true for the feature test wirings and for the library ones. Once a helper bundles more than a single value, it is a `Wired`. Do not invent a new name for it in each file.
- When two backends must share a behaviour, that behaviour gets a **conformance suite**. A conformance suite has three parts: free functions that take `&impl Trait`, a `Workbench` trait (`setup`/`store`/`cleanup`), and a `conformance_tests!` macro that each backend invokes. So there is one list of cases, and both implementations run it.
- If a guard is worth having, it is worth proving that it is load-bearing. Break the guard on purpose and watch the test fail.
- Browser tests run side by side, against one server: twelve at a time (four with accounts), or two at a time on CI. Each test works in a project of its own, with a name that no other test uses. A test never assumes it is alone. So it never counts all projects, and it never relies on what the server held before the test started.
- On a developer's machine, a failed browser test is run once more. If it passes the second time, it is reported as **flaky**, not as passed. Treat a flaky test as a finding. The retry exists because of Windows: Windows keeps the connections of a whole run open for minutes. So a run started right after another run can fail to load a page (`ERR_NO_BUFFER_SPACE`). CI never retries.
- A browser test that reloads the page must first wait for whatever the reload will read. The reason is that the client shows a change before the change is stored. Things to wait for:
  - the ideas catalog (`catalogued`, `uncatalogued`; `captureIdea` already waits for it),
  - a board change being saved (`boardSaves`),
  - a scene's prose reaching the server (poll its text).

  A missing wait shows up in local mode, because there the catalogs lag a moment behind.
- Browser tests run against a **fresh build**. Playwright starts `cargo leptos serve --split`. That command builds the client and the server before the server starts listening. So no test runs against an old client. If a mutation check passes within a second, that is the sign that it did not run against a fresh build. The tests' server runs at :3100, from `target/site-tests`. It is separate from a `cargo dev` at :3000. If an interrupted run left a server at :3100, that server is reused as it is. So stop it before you trust a run.

## Rust

- Import types instead of qualifying them. Write `use sqlx::postgres::PgListener;`, never `sqlx::postgres::PgListener` inline. The one exception is a one-off `std::time::Duration` next to a `time::Duration` that is already in scope. Traits, enums and constants are imported the same way.
- Call a free function through its module. Never import the function by itself. Examples: `use crate::speech;` and then `speech::said("sign-out")`, `eventpublishing::message_for(happened, body)`, `routing::get(handler)`.
  - The path shows where the function lives.
  - Two functions with the same name need no alias. An example is a feature's `message_for` and the library's `message_for` that it wraps.
  - Sometimes the module's own name clashes with something else: with a crate of that name (`tokio::time` next to `time`), or with another module (`outbox::postgres` next to `crate::postgres`). Then the call uses the longer path.
  - This rule does not touch: associated functions (`Account::from_first`), which already carry their type; macros (`json!`); attributes; preludes that are meant to be glob-imported (`leptos::prelude::*`); and a test module's `use super::*`.
- Use `thiserror` for every error type. Use `#[error(transparent)]` when wrapping another error whole.
- Every `expect` states the reason why it cannot fail: `.expect("a plain title is fine")`.
- Modules and re-exports that sit behind a cargo feature come last. They follow a blank line and are grouped by feature. This keeps what a build may leave out apart from what every build has. A feature's test module sits in that feature's group. The blank line also stops rustfmt from sorting them back in among the others.

## Writes that hold together

These points are checked in the review of every slice that writes data or sends messages. Each point is a fault that both integrity audits found more than once.

- **More than one port, one transaction.** A write may touch two stores. Examples are an event and its secret, or a row and its outbox entry. Such a write happens in one transaction. If that is not possible yet, a `// TODO` says why.
- **Check and act together.** Some checks decide whether a write happens: does it exist, is it still open, is this user allowed. Such a check is made inside the writing transaction, or under a lock that the other writer also takes. It is never made before the transaction.
- **Effects after the commit.** Some things happen after a write: a message is sent, an event is published, or the client navigates somewhere. These things happen only after the write has committed. Ask what is left if the process dies in between. Ask whether the next pass or a retry finishes the work.
- **Received twice, received late.** A message may arrive a second time. It may also arrive after a newer message. The listener changes nothing the second time, and it keeps the newer version.
- **Answers out of order.** In the client, asking again keeps only the newest answer (`Asking`). A press cannot start something twice (`Act`).

## Client

- What an author changes in an aggregate goes through the aggregate's **`WorkingCopy`**. For a list of aggregates, it goes through **`WorkingCopies`**. See `clients/web/src/working_copy.rs` and [ARCHITECTURE](./ARCHITECTURE.md#what-the-client-changes-and-what-it-is-told--decided). A change is:
  - shown at once,
  - sent one at a time with `If-Match`,
  - merged with an older change, where the newer change makes the older one unnecessary.

  More rules for working copies:
  - A button that starts a change shows `aria-busy` from `unanswered()` or `unanswered_on(&id)`.
  - Code that waits on a change awaits `intended(..)`. That resolves to `Ok` or to the refusal. The waiting code handles the refusal itself. `failed` only receives refusals of changes that nobody waits on.
  - A change may no longer fit what it was made for. Its intent's `still_fits` says so. Such a change is refused and is not tried again.
  - A list is read with `read_all`. One item is read again with `read_again(&id)`. An item is ended with `ending`.
  - One aggregate is followed with `following` and `heard`. A view that notes changes made by others does that in `noticing_others`. It never does it for answers to its own changes.
  - Every aggregate uses a working copy. An `Act` remains only for things that create something or change nothing the client holds: signing in, signing up, a code, a reset. An `Asking` remains for reading again what no working copy holds.
- Some things are started once by a click or a key press and ask the server. Examples are saving, inviting, signing in, and writing in a section. Such a thing is an **`Act`** (`clients/web/src/act.rs`). An act is built once, with its body. It is started with `act.start(about)`. While it runs, starting it again does nothing. So a double click or a held key never runs it twice. Do not keep a flag of your own, and do not write `if busy { return }`. A run of small changes that are meant to add up is different. An example is a card moved by a held arrow key. Such changes are queued instead, not dropped.
- The button that starts an act shows that the act runs, with `.attr("aria-busy", move || act.busy())`. Where one act serves one button per row, it uses `busy_on(&about)`. `disabled` is kept for things that may not be done at all. `aria-busy` is what draws the sweeping line. The line appears only after 150 ms, so a quick answer never flickers.
- Some things an act does after its answer comes only make sense on the page the act was started from. Navigating elsewhere is the main example. Such things happen only if `here.still()` is true.
- Asking again for something that changes is an **`Asking`** (`clients/web/src/asking.rs`). Examples are a list, the open project, and one's roles. Call `asking.next()` before the request. Its answer is taken in only if `asking.is_latest(asked)` is true. So an answer that was read before a change, but arrives after it, never replaces what came later. The difference between the two: an `Act` stops a second press from starting. An `Asking` lets every request start and keeps only the newest answer.

## HTTP

- A feature owns `/api/{its noun}`, in the plural: `/api/ideas`, `/api/boards`. Anything the service serves about itself, such as its health or the messages it refused, lives under `/api/service/`. So no feature may be called `service`. A path says whose endpoint it is.

## Manifests

- Dependencies are grouped in this order: workspace crates, then this feature's own crates, then **foreign contracts**. The foreign contracts sit under a comment that says what they are. A feature may depend on another feature's `contract` crate, and on nothing else of that feature. It never depends on the other feature's `core`, and never on its adapters. The grouping makes a mistake obvious at a glance, for example an `ideas-core` in a `boards` manifest.

## SQL

- Statements are `const` items in SCREAMING_SNAKE case. Each one sits directly above the function that runs it. When a file grows past a couple of operations, give each operation its own file, with its constant next to it.
- Id columns are `TEXT COLLATE "C"`. Base62 ids only sort by age under byte order. A glibc locale would put `a` before `A`.
- Every schema change is a migration file. `sqlx::migrate!` embeds the migrations **at compile time**. It cannot tell cargo that the `.sql` files are inputs. So to make a new migration take effect, touch a `.rs` file in the crate.
